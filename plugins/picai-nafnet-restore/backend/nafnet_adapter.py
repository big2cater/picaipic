from __future__ import annotations

import os
import queue
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Any

import cv2
import numpy as np

from denoiser import NAFNET_TASKS, NAFNetRestorer, restore_image


PLUGIN_ID = "picai-nafnet-restore"
PLUGIN_ROOT = Path(os.environ.get("PICAIPIC_PLUGIN_ROOT", "."))
PLUGIN_MODEL_ROOT = Path(os.environ.get("PICAIPIC_PLUGIN_MODEL_DIR", PLUGIN_ROOT / "models"))
DEFAULT_SOURCE_ROOT = Path(
    os.environ.get("NAFNET_SOURCE_DIR")
    or PLUGIN_MODEL_ROOT.joinpath("nafnet")
)
TERMINAL_STATUSES = {"succeeded", "failed", "cancelled"}
DEFAULT_TASK_HISTORY_LIMIT = 500
VALID_STATUS_TRANSITIONS = {
    None: {"queued", "running", "failed", "cancelled"},
    "queued": {"running", "cancelling", "failed", "cancelled"},
    "running": {"cancelling", "succeeded", "failed", "cancelled"},
    "cancelling": {"cancelled", "failed", "succeeded"},
    "succeeded": set(),
    "failed": set(),
    "cancelled": set(),
}
CAPABILITY_TO_TASK = {
    "denoise": "denoise",
    "deblur": "deblur",
    "jpeg-artifact-removal": "jpeg",
}


def _input_path(payload: dict[str, Any], *names: str) -> str | None:
    inputs = payload.get("inputs") or {}
    for name in names:
        item = inputs.get(name)
        if isinstance(item, dict) and item.get("path"):
            return str(item["path"])
        if isinstance(item, str):
            return item
    return None


def _safe_stem(path: str) -> str:
    stem = Path(path).stem or "image"
    keep = [char if char.isalnum() or char in ("-", "_") else "_" for char in stem]
    return "".join(keep)[:80] or "image"


def _error_domain(exc: BaseException) -> tuple[str, str]:
    text = str(exc).lower()
    if "task cancelled" in text:
        return "TASK_CANCELLED", "task"
    if "out of memory" in text or "hip_out_of_memory" in text or "cuda out of memory" in text:
        return "DEVICE_OOM", "device_backend"
    if isinstance(exc, PermissionError):
        return "PERMISSION_DENIED", "filesystem"
    if isinstance(exc, FileNotFoundError):
        return "FILE_NOT_FOUND", "filesystem"
    return "TASK_FAILED", "plugin"


def _int_parameter(parameters: dict[str, Any], name: str, default: int, minimum: int, maximum: int) -> int:
    try:
        value = int(parameters.get(name, default))
    except (TypeError, ValueError):
        value = default
    return max(minimum, min(maximum, value))


def _float_parameter(parameters: dict[str, Any], name: str, default: float, minimum: float, maximum: float) -> float:
    try:
        value = float(parameters.get(name, default))
    except (TypeError, ValueError):
        value = default
    return max(minimum, min(maximum, value))


def _bool_parameter(parameters: dict[str, Any], name: str, default: bool = False) -> bool:
    value = parameters.get(name, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.lower() in ("1", "true", "yes", "on")
    return bool(value)


def _file_state(path: Path) -> dict[str, Any]:
    exists = path.exists() and path.is_file()
    return {
        "path": str(path),
        "available": exists,
        "exists": exists,
        "sizeBytes": path.stat().st_size if exists else 0,
    }


class NAFNetAdapter:
    def __init__(self) -> None:
        self.source_root = DEFAULT_SOURCE_ROOT
        self._lock = threading.Lock()
        self._cancelled_tasks: set[str] = set()
        self._task_states: dict[str, dict[str, Any]] = {}
        self._task_events: dict[str, list[dict[str, Any]]] = {}
        self._task_seq = 0
        self._task_history_limit = _int_parameter(
            {"value": os.environ.get("PICAIPIC_TASK_HISTORY_LIMIT")},
            "value",
            DEFAULT_TASK_HISTORY_LIMIT,
            50,
            10000,
        )
        self._task_condition = threading.Condition(self._lock)
        self._task_queue: queue.Queue[tuple[str, str, dict[str, Any]]] = queue.Queue()
        self._active_task_id: str | None = None
        self._restorer = NAFNetRestorer(self.source_root)
        self._last_error: dict[str, Any] | None = None
        self._worker = threading.Thread(target=self._worker_loop, name="nafnet-worker", daemon=True)
        self._worker.start()

    def status(self) -> dict[str, Any]:
        torch_state = self._torch_state()
        restorer_status = self._restorer.status()
        models = [
            {"id": task, **_file_state(self._restorer.weights_path(task)), "loaded": task in self._restorer.models}
            for task in NAFNET_TASKS
        ]
        source_ready = self.source_root.exists()
        runtime_ready = torch_state["available"]
        any_model_ready = any(model["available"] for model in models)
        ready = bool(source_ready and runtime_ready and any_model_ready)
        reason = None
        if not source_ready:
            reason = "source_missing"
        elif not runtime_ready:
            reason = "dependency_missing"
        elif not any_model_ready:
            reason = "model_missing"
        capability_states = {}
        for capability, task in CAPABILITY_TO_TASK.items():
            model_ready = self._restorer.weights_path(task).is_file()
            available = bool(source_ready and runtime_ready and model_ready)
            capability_reason = None
            if not source_ready:
                capability_reason = "source_missing"
            elif not runtime_ready:
                capability_reason = "dependency_missing"
            elif not model_ready:
                capability_reason = "model_missing"
            capability_states[capability] = {
                "available": available,
                "reason": capability_reason,
            }

        return {
            "pluginId": PLUGIN_ID,
            "ready": ready,
            "reason": reason,
            "source": {
                "path": str(self.source_root),
                "available": source_ready,
            },
            "environment": {
                "runtime": "python",
                "pythonVersion": sys.version.split()[0],
                "torch": torch_state,
            },
            "selectedDevice": restorer_status.get("device"),
            "models": models,
            "capabilities": capability_states,
            "lastError": self._last_error,
            "taskQueue": {
                "activeTaskId": self._active_task_id,
                "queuedCount": self._task_queue.qsize(),
                "maxActiveGpuTasks": 1,
            },
        }

    def diagnostics(self) -> dict[str, Any]:
        return {
            "pluginId": PLUGIN_ID,
            "status": self.status(),
            "logFiles": ["logs/plugin.log"],
        }

    def smoke_test(self, payload: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        started = time.time()
        profile_id = str(payload.get("profileId") or "")
        backend = str(payload.get("backend") or "auto").lower()
        capability = str(payload.get("capability") or "denoise")
        task = CAPABILITY_TO_TASK.get(capability, capability)
        steps: list[dict[str, Any]] = []

        def add_step(name: str, passed: bool, details: Any | None = None, error: str | None = None) -> None:
            step: dict[str, Any] = {"name": name, "passed": passed}
            if details is not None:
                step["details"] = details
            if error:
                step["error"] = error
            steps.append(step)

        status = self.status()
        add_step("python", True, {"executable": sys.executable, "version": sys.version.split()[0]})
        task_known = task in NAFNET_TASKS
        add_step("capability", task_known, {"requested": capability, "task": task}, None if task_known else "Unsupported NAFNet capability")

        # Product default for denoise is the fast OpenCV path. Packaged installs
        # should pass Smoke without a full NAFNet checkout or model weights.
        if task == "denoise":
            try:
                tiny = np.full((32, 32, 3), 127, dtype=np.uint8)
                restored, meta = restore_image(
                    tiny,
                    self._restorer,
                    task="denoise",
                    method="opencv",
                    device=backend,
                )
                opencv_ok = bool(restored.shape == tiny.shape and restored.dtype == np.uint8)
                add_step(
                    "opencv-fast",
                    opencv_ok,
                    {"shape": list(restored.shape), "meta": meta},
                    None if opencv_ok else "OpenCV fast denoise returned an invalid image",
                )
            except Exception as exc:
                self._last_error = {"code": "smoke_test_failed", "message": str(exc)}
                add_step("opencv-fast", False, None, str(exc))

            passed = all(step["passed"] for step in steps)
            duration_ms = int((time.time() - started) * 1000)
            response = {
                "ok": passed,
                "passed": passed,
                "pluginId": PLUGIN_ID,
                "profileId": profile_id,
                "backend": backend,
                "capability": capability,
                "durationMs": duration_ms,
                "environment": status["environment"],
                "models": status["models"],
                "steps": steps,
            }
            if not passed:
                error_steps = [step for step in steps if not step["passed"]]
                response["error"] = {
                    "code": "smoke_test_failed",
                    "message": error_steps[0].get("error") or f"Smoke test step failed: {error_steps[0]['name']}",
                    "failedSteps": [step["name"] for step in error_steps],
                }
            return (200 if passed else 503), response

        source_available = bool(status["source"]["available"])
        add_step("source", source_available, status["source"], None if source_available else "NAFNet source directory is missing")
        torch_state = status["environment"]["torch"]
        torch_available = bool(torch_state.get("available"))
        add_step("torch", torch_available, torch_state, None if torch_available else str(torch_state.get("error") or "PyTorch is unavailable"))
        model_ready = task_known and self._restorer.weights_path(task).is_file()
        add_step(
            "model",
            bool(model_ready),
            self._restorer.task_status(task) if task_known else None,
            None if model_ready else "Required NAFNet weights are missing",
        )
        backend_ok = self._backend_matches(backend, torch_state)
        add_step(
            "backend",
            backend_ok,
            {"requested": backend, "torch": torch_state},
            None if backend_ok else f"Requested backend is not available: {backend}",
        )

        can_load = source_available and torch_available and task_known and model_ready and backend_ok
        if can_load:
            try:
                self._restorer.load(task, requested_device=backend)
                add_step("load-model", True, self._restorer.task_status(task))
            except Exception as exc:
                self._last_error = {"code": "smoke_test_failed", "message": str(exc)}
                add_step("load-model", False, None, str(exc))

        passed = all(step["passed"] for step in steps)
        duration_ms = int((time.time() - started) * 1000)
        response = {
            "ok": passed,
            "passed": passed,
            "pluginId": PLUGIN_ID,
            "profileId": profile_id,
            "backend": backend,
            "capability": capability,
            "durationMs": duration_ms,
            "environment": status["environment"],
            "models": status["models"],
            "steps": steps,
        }
        if not passed:
            error_steps = [step for step in steps if not step["passed"]]
            response["error"] = {
                "code": "smoke_test_failed",
                "message": error_steps[0].get("error") or f"Smoke test step failed: {error_steps[0]['name']}",
                "failedSteps": [step["name"] for step in error_steps],
            }
        return (200 if passed else 503), response

    def invoke(self, capability: str, payload: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        if capability not in CAPABILITY_TO_TASK:
            return self._error(404, str(payload.get("taskId") or ""), "CAPABILITY_NOT_FOUND", f"Unknown capability: {capability}")
        task_id = str(payload.get("taskId") or uuid.uuid4())
        payload = dict(payload)
        payload["taskId"] = task_id
        self._set_task_state(
            task_id,
            "queued",
            capability=capability,
            queuePosition=self._task_queue.qsize() + 1,
        )
        self._task_queue.put((task_id, capability, payload))
        return 202, {
            "ok": True,
            "async": True,
            "pluginId": PLUGIN_ID,
            "taskId": task_id,
            "status": "queued",
            "events": {
                "method": "GET",
                "path": f"/tasks/{task_id}/events",
                "cursor": 0,
                "timeoutMs": 25000,
            },
            "poll": {
                "method": "GET",
                "path": f"/tasks/{task_id}",
                "intervalMs": 1000,
            },
        }

    def cancel_task(self, task_id: str) -> tuple[int, dict[str, Any]]:
        if not task_id:
            return self._error(400, task_id, "INVALID_TASK", "taskId is required")
        with self._lock:
            state = dict(self._task_states.get(task_id) or {})
            if not state:
                return 404, {
                    "ok": False,
                    "taskId": task_id,
                    "status": "unknown",
                    "error": {
                        "code": "TASK_NOT_FOUND",
                        "domain": "task",
                        "message": f"Task is not known to the plugin: {task_id}",
                    },
                }
            if state.get("status") not in TERMINAL_STATUSES:
                self._cancelled_tasks.add(task_id)
        if state.get("status") in TERMINAL_STATUSES:
            next_status = str(state.get("status"))
            message = state.get("message")
        elif state.get("status") == "queued":
            next_status = "cancelled"
            message = "Cancelled before execution"
        else:
            next_status = "cancelling"
            message = "Cancelling at next checkpoint"
        self._set_task_state(task_id, next_status, message=message)
        return 200, {"ok": True, "taskId": task_id, "status": next_status}

    def task_status(self, task_id: str) -> tuple[int, dict[str, Any]]:
        if not task_id:
            return self._error(400, task_id, "INVALID_TASK", "taskId is required")
        with self._lock:
            state = dict(self._task_states.get(task_id) or {})
            cancelled = task_id in self._cancelled_tasks
        if not state:
            return 404, {
                "ok": False,
                "taskId": task_id,
                "status": "unknown",
                "error": {
                    "code": "TASK_NOT_FOUND",
                    "domain": "task",
                    "message": f"Task is not known to the plugin: {task_id}",
                },
            }
        if cancelled and state.get("status") in ("queued", "running"):
            state["status"] = "cancelling"
        return 200, {"ok": True, "taskId": task_id, **state}

    def task_events(self, task_id: str, after: int = 0, timeout_ms: int = 25000) -> tuple[int, dict[str, Any]]:
        if not task_id:
            return self._error(400, task_id, "INVALID_TASK", "taskId is required")
        timeout_ms = max(0, min(timeout_ms, 30000))
        deadline = time.monotonic() + (timeout_ms / 1000)
        with self._task_condition:
            while True:
                events = [event for event in self._task_events.get(task_id, []) if int(event.get("seq") or 0) > after]
                state = dict(self._task_states.get(task_id) or {})
                if events or state.get("status") in TERMINAL_STATUSES:
                    break
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                self._task_condition.wait(timeout=remaining)

            if not state and task_id not in self._task_events:
                return 404, {
                    "ok": False,
                    "taskId": task_id,
                    "status": "unknown",
                    "events": [],
                    "nextCursor": after,
                    "error": {
                        "code": "TASK_NOT_FOUND",
                        "domain": "task",
                        "message": f"Task is not known to the plugin: {task_id}",
                    },
                }

        next_cursor = after
        if events:
            next_cursor = max(int(event.get("seq") or after) for event in events)
        return 200, {
            "ok": True,
            "taskId": task_id,
            "status": state.get("status"),
            "terminal": state.get("status") in TERMINAL_STATUSES,
            "events": events,
            "nextCursor": next_cursor,
            "state": state,
        }

    def _run_restore_task(self, capability: str, payload: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        task_id = str(payload.get("taskId") or "")
        task = CAPABILITY_TO_TASK[capability]
        self._set_task_state(task_id, "running", capability=capability, progress=5, message="Starting")
        source_path = _input_path(payload, "source", "content")
        output_dir = Path(str(payload.get("outputDir") or os.getcwd()))
        parameters = payload.get("parameters") or {}
        if _bool_parameter(parameters, "mockTask"):
            return self._run_mock_task(task_id, output_dir, parameters)
        output_format = str(parameters.get("outputFormat") or "png").lower()
        if output_format not in ("png", "jpg", "jpeg"):
            output_format = "png"
        ext = "jpg" if output_format == "jpeg" else output_format

        if not source_path:
            return self._error(400, task_id, "invalid_input", "inputs.source.path is required")
        if not Path(source_path).is_file():
            return self._error(400, task_id, "invalid_input", f"Source image does not exist: {source_path}")

        if not self._capability_available(capability) and str(parameters.get("method") or "auto").lower() == "nafnet":
            return self._error(503, task_id, "plugin_not_ready", f"NAFNet is not ready for {capability}", self.status())

        output_dir.mkdir(parents=True, exist_ok=True)
        result_path = output_dir / f"{_safe_stem(source_path)}-{task}-{task_id[:8]}.{ext}"

        try:
            self._raise_if_cancelled(task_id)
            started = time.time()
            self._set_task_progress(task_id, 12, "Reading image")
            img = cv2.imread(str(source_path), cv2.IMREAD_COLOR)
            if img is None:
                raise ValueError(f"Cannot read image: {source_path}")
            self._raise_if_cancelled(task_id)
            self._set_task_progress(task_id, 25, "Restoring image")
            result, meta = restore_image(
                img,
                self._restorer,
                task=task,
                method=str(parameters.get("method") or "auto"),
                device=str(parameters.get("device") or "auto"),
                strength=_float_parameter(parameters, "strength", 0.55, 0.0, 1.0),
                detail=_float_parameter(parameters, "detail", 0.65, 0.0, 1.0),
                sharpen=_float_parameter(parameters, "sharpen", 0.15, 0.0, 1.0),
            )
            self._raise_if_cancelled(task_id)
            self._set_task_progress(task_id, 85, "Writing output")
            if ext == "jpg":
                ok = cv2.imwrite(str(result_path), result, [int(cv2.IMWRITE_JPEG_QUALITY), 95])
            else:
                ok = cv2.imwrite(str(result_path), result)
            if not ok:
                raise RuntimeError(f"Failed to write output: {result_path}")
            elapsed_ms = int((time.time() - started) * 1000)
        except Exception as exc:
            code, domain = _error_domain(exc)
            status = "cancelled" if code == "TASK_CANCELLED" else "failed"
            self._set_task_state(task_id, status, error={"code": code, "domain": domain, "message": str(exc)})
            return self._error(500, task_id, code, str(exc), {"domain": domain})

        output = {
            "id": "result",
            "kind": "image",
            "path": str(result_path),
            "mime": "image/jpeg" if ext == "jpg" else "image/png",
            "sourceInputId": "source",
        }
        final_meta = {
            "capability": capability,
            "task": task,
            "requestedDevice": parameters.get("device", "auto"),
            "elapsedMs": elapsed_ms,
            **meta,
        }
        self._set_task_state(task_id, "succeeded", outputs=[output], meta=final_meta, progress=100, message="Completed")
        return 200, {"ok": True, "taskId": task_id, "status": "succeeded", "outputs": [output], "meta": final_meta}

    def _run_mock_task(self, task_id: str, output_dir: Path, parameters: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        output_dir.mkdir(parents=True, exist_ok=True)
        duration_ms = _int_parameter(parameters, "mockDurationMs", 2000, 0, 120000)
        step_ms = _int_parameter(parameters, "mockStepMs", 100, 25, 5000)
        started = time.time()
        elapsed_ms = 0
        try:
            while elapsed_ms < duration_ms:
                self._raise_if_cancelled(task_id)
                time.sleep(min(step_ms, duration_ms - elapsed_ms) / 1000)
                elapsed_ms = int((time.time() - started) * 1000)
                progress = 100 if duration_ms == 0 else min(99, int((elapsed_ms / duration_ms) * 100))
                self._set_task_progress(task_id, progress, f"Mock task running: {elapsed_ms}/{duration_ms} ms")
            self._raise_if_cancelled(task_id)
        except Exception as exc:
            code, domain = _error_domain(exc)
            self._set_task_state(task_id, "cancelled" if code == "TASK_CANCELLED" else "failed", error={"code": code, "domain": domain, "message": str(exc)})
            return self._error(500, task_id, code, str(exc), {"domain": domain})

        result_path = output_dir / f"mock-task-{_safe_stem(task_id)}.txt"
        result_path.write_text(f"mock task {task_id} completed in {elapsed_ms} ms\n", encoding="utf-8")
        output = {"id": "result", "kind": "text", "path": str(result_path), "mime": "text/plain"}
        self._set_task_state(task_id, "succeeded", outputs=[output], meta={"mock": True, "elapsedMs": elapsed_ms}, progress=100, message="Completed")
        return 200, {"ok": True, "taskId": task_id, "status": "succeeded", "outputs": [output], "meta": {"mock": True, "elapsedMs": elapsed_ms}}

    def _worker_loop(self) -> None:
        while True:
            task_id, capability, payload = self._task_queue.get()
            try:
                with self._lock:
                    self._active_task_id = task_id
                    cancelled = task_id in self._cancelled_tasks
                    current_status = (self._task_states.get(task_id) or {}).get("status")
                if current_status in TERMINAL_STATUSES:
                    continue
                if cancelled:
                    self._set_task_state(task_id, "cancelled", error={
                        "code": "TASK_CANCELLED",
                        "domain": "task",
                        "message": f"Task cancelled before execution: {task_id}",
                    })
                    continue
                status, response = self._run_restore_task(capability, payload)
                if status >= 400 or response.get("ok") is False:
                    error = dict(response.get("error") or {})
                    code = str(error.get("code") or "TASK_FAILED")
                    if code.upper() == "TASK_CANCELLED":
                        continue
                    if "domain" not in error:
                        error["domain"] = "plugin"
                    self._set_task_state(task_id, "failed", error=error)
            except Exception as exc:
                code, domain = _error_domain(exc)
                self._set_task_state(
                    task_id,
                    "cancelled" if code == "TASK_CANCELLED" else "failed",
                    error={"code": code, "domain": domain, "message": str(exc)},
                )
            finally:
                with self._lock:
                    if self._active_task_id == task_id:
                        self._active_task_id = None
                self._task_queue.task_done()

    def _set_task_state(self, task_id: str, status: str, **extra: Any) -> None:
        if not task_id:
            return
        now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        with self._task_condition:
            current = dict(self._task_states.get(task_id) or {})
            previous = current.get("status")
            if status == previous and not extra:
                return
            allowed = VALID_STATUS_TRANSITIONS.get(previous, set())
            if status != previous and status not in allowed:
                attempted = status
                status = "failed"
                extra = {
                    "error": {
                        "code": "INVALID_TASK_TRANSITION",
                        "domain": "task",
                        "message": f"Invalid task transition: {previous or 'none'} -> {attempted}",
                    }
                }
            if not current:
                current["createdAt"] = now
            current["status"] = status
            current["updatedAt"] = now
            current.update(extra)
            self._task_states[task_id] = current
            if status in TERMINAL_STATUSES:
                self._cancelled_tasks.discard(task_id)
            self._task_seq += 1
            event = {
                "seq": self._task_seq,
                "taskId": task_id,
                "type": "task.state",
                "status": status,
                "previousStatus": previous,
                "at": now,
                "state": dict(current),
            }
            events = self._task_events.setdefault(task_id, [])
            events.append(event)
            if len(events) > 200:
                del events[:-200]
            self._prune_task_history_locked()
            self._task_condition.notify_all()

    def _set_task_progress(self, task_id: str, progress: int, message: str | None = None) -> None:
        if not task_id:
            return
        now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        progress = max(0, min(100, int(progress)))
        with self._task_condition:
            current = dict(self._task_states.get(task_id) or {})
            status = current.get("status")
            if not current or status in TERMINAL_STATUSES:
                return
            current["progress"] = progress
            if message is not None:
                current["message"] = message
            current["updatedAt"] = now
            self._task_states[task_id] = current
            self._task_seq += 1
            event = {
                "seq": self._task_seq,
                "taskId": task_id,
                "type": "task.progress",
                "status": status,
                "progress": progress,
                "message": message,
                "at": now,
                "state": dict(current),
            }
            events = self._task_events.setdefault(task_id, [])
            events.append(event)
            if len(events) > 200:
                del events[:-200]
            self._task_condition.notify_all()

    def _prune_task_history_locked(self) -> None:
        if len(self._task_states) <= self._task_history_limit:
            return
        terminal_items = [
            (str(state.get("updatedAt") or ""), task_id)
            for task_id, state in self._task_states.items()
            if state.get("status") in TERMINAL_STATUSES and task_id != self._active_task_id
        ]
        terminal_items.sort()
        overflow = len(self._task_states) - self._task_history_limit
        for _, task_id in terminal_items[:overflow]:
            self._task_states.pop(task_id, None)
            self._task_events.pop(task_id, None)
            self._cancelled_tasks.discard(task_id)

    def _raise_if_cancelled(self, task_id: str) -> None:
        if not task_id:
            return
        with self._lock:
            cancelled = task_id in self._cancelled_tasks
        if cancelled:
            raise RuntimeError(f"Task cancelled: {task_id}")

    def _torch_state(self) -> dict[str, Any]:
        try:
            import torch

            cuda_available = bool(torch.cuda.is_available())
            device_name = torch.cuda.get_device_name(0) if cuda_available else None
            return {
                "available": True,
                "version": getattr(torch, "__version__", None),
                "cudaAvailable": cuda_available,
                "cudaDeviceName": device_name,
            }
        except Exception as exc:
            return {"available": False, "error": str(exc)}

    def _backend_matches(self, backend: str, torch_state: dict[str, Any]) -> bool:
        requested = (backend or "auto").lower()
        if requested in ("auto", ""):
            return bool(torch_state.get("available"))
        if requested == "cpu":
            return bool(torch_state.get("available"))
        if requested in ("cuda", "rocm"):
            return bool(torch_state.get("available") and torch_state.get("cudaAvailable"))
        return bool(torch_state.get("available"))

    def _capability_available(self, capability: str) -> bool:
        task = CAPABILITY_TO_TASK.get(capability)
        if not task:
            return False
        status = self.status_shallow()
        return bool(status["source"] and status["torch"] and self._restorer.weights_path(task).is_file())

    def status_shallow(self) -> dict[str, Any]:
        return {
            "source": self.source_root.exists(),
            "torch": self._torch_state().get("available"),
        }

    def _error(self, http_status: int, task_id: str, code: str, message: str, details: Any | None = None) -> tuple[int, dict[str, Any]]:
        error: dict[str, Any] = {
            "code": code,
            "message": message,
        }
        if details is not None:
            error["details"] = details
        self._last_error = error
        return http_status, {
            "ok": False,
            "pluginId": PLUGIN_ID,
            "taskId": task_id,
            "status": "failed",
            "error": error,
        }

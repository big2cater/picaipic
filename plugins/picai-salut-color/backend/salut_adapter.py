from __future__ import annotations

import os
import queue
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Any


PLUGIN_ID = "picai-salut-color"
PLUGIN_ROOT = Path(os.environ.get("PICAIPIC_PLUGIN_ROOT", "."))
PLUGIN_MODEL_ROOT = Path(os.environ.get("PICAIPIC_PLUGIN_MODEL_DIR", PLUGIN_ROOT / "models"))
DEFAULT_SOURCE_ROOT = Path(
    os.environ.get("SALUT_SOURCE_DIR")
    or os.environ.get("PICAIPIC_WINDOWS_SALUT_BACKEND")
    or Path(__file__).resolve().parent
)
DEFAULT_SOURCE_MODEL_DIR = DEFAULT_SOURCE_ROOT / "models" / "salut"
DEFAULT_MODEL_DIR = Path(
    os.environ.get("SALUT_MODEL_DIR")
    or (DEFAULT_SOURCE_MODEL_DIR if DEFAULT_SOURCE_MODEL_DIR.exists() else PLUGIN_MODEL_ROOT / "salut")
)
DEFAULT_CKPT_PATH = Path(
    os.environ.get("SALUT_CKPT_PATH")
    or DEFAULT_MODEL_DIR / "epoch=100-step=4127466.ckpt.state.pt"
)
DEFAULT_VGG_PATH = Path(
    os.environ.get("SALUT_VGG_PATH")
    or DEFAULT_MODEL_DIR / "vgg_normalised.pth"
)
LFS_PREFIX = b"version https://git-lfs.github.com/spec/v1"
TERMINAL_STATUSES = {"succeeded", "failed", "cancelled"}
DEFAULT_TASK_HISTORY_LIMIT = 500
RAW_IMAGE_EXTENSIONS = {
    ".3fr",
    ".arw",
    ".cr2",
    ".cr3",
    ".dng",
    ".erf",
    ".fff",
    ".iiq",
    ".kdc",
    ".mef",
    ".mos",
    ".mrw",
    ".nef",
    ".nrw",
    ".orf",
    ".pef",
    ".raf",
    ".raw",
    ".rw2",
    ".rwl",
    ".sr2",
    ".srf",
    ".x3f",
}
VALID_STATUS_TRANSITIONS = {
    None: {"queued", "running", "failed", "cancelled"},
    "queued": {"running", "cancelling", "failed", "cancelled"},
    "running": {"cancelling", "succeeded", "failed", "cancelled"},
    "cancelling": {"cancelled", "failed", "succeeded"},
    "succeeded": set(),
    "failed": set(),
    "cancelled": set(),
}


def _is_lfs_pointer(path: Path) -> bool:
    if not path.exists() or not path.is_file():
        return False
    try:
        with path.open("rb") as handle:
            return handle.read(len(LFS_PREFIX)) == LFS_PREFIX
    except OSError:
        return False


def _file_state(path: Path) -> dict[str, Any]:
    exists = path.exists() and path.is_file()
    is_lfs = _is_lfs_pointer(path) if exists else False
    size = path.stat().st_size if exists else 0
    return {
        "path": str(path),
        "available": exists and not is_lfs,
        "exists": exists,
        "isGitLfsPointer": is_lfs,
        "sizeBytes": size,
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
    keep = []
    for char in stem:
        keep.append(char if char.isalnum() or char in ("-", "_") else "_")
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


def _bool_parameter(parameters: dict[str, Any], name: str, default: bool = False) -> bool:
    value = parameters.get(name, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.lower() in ("1", "true", "yes", "on")
    return bool(value)


class SalutAdapter:
    def __init__(self) -> None:
        configured_backend = os.environ.get("PICAIPIC_WINDOWS_SALUT_BACKEND")
        self.source_root = Path(configured_backend) if configured_backend else Path(os.environ.get("SALUT_SOURCE_DIR", str(DEFAULT_SOURCE_ROOT)))
        self.ckpt_path = Path(os.environ.get("SALUT_CKPT_PATH", str(DEFAULT_CKPT_PATH)))
        self.vgg_path = Path(os.environ.get("SALUT_VGG_PATH", str(DEFAULT_VGG_PATH)))
        self.force_cpu = os.environ.get("SALUT_FORCE_CPU", "").lower() in ("1", "true", "yes")
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
        self._task_queue: queue.Queue[tuple[str, dict[str, Any]]] = queue.Queue()
        self._active_task_id: str | None = None
        self._model_lock = threading.Lock()
        self._model = None
        self._device = None
        self._device_info: dict[str, Any] | None = None
        self._last_error: dict[str, Any] | None = None
        self._loaded_at: float | None = None
        self._worker = threading.Thread(target=self._worker_loop, name="salut-worker", daemon=True)
        self._worker.start()

    def status(self) -> dict[str, Any]:
        torch_state = self._torch_state()
        models = [
            {"id": "salut-main", **_file_state(self.ckpt_path), "loaded": self._model is not None},
            {"id": "vgg-normalised", **_file_state(self.vgg_path), "loaded": self._model is not None},
        ]
        models_ready = all(model["available"] for model in models)
        runtime_ready = torch_state["available"]
        source_ready = self.source_root.exists()
        ready = bool(source_ready and models_ready and runtime_ready)
        reason = None
        if not source_ready:
            reason = "source_missing"
        elif not models_ready:
            reason = "model_missing"
        elif not runtime_ready:
            reason = "dependency_missing"

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
            "selectedDevice": self._device,
            "deviceInfo": self._device_info,
            "models": models,
            "capabilities": {
                "color-transfer": {
                    "available": ready,
                    "reason": None if ready else reason,
                },
                "export-lut": {
                    "available": False,
                    "reason": "not_implemented",
                },
            },
            "lastError": self._last_error,
            "loadedAt": self._loaded_at,
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
        capability = str(payload.get("capability") or "color-transfer")
        steps: list[dict[str, Any]] = []

        def add_step(name: str, passed: bool, details: Any | None = None, error: str | None = None) -> None:
            step: dict[str, Any] = {
                "name": name,
                "passed": passed,
            }
            if details is not None:
                step["details"] = details
            if error:
                step["error"] = error
            steps.append(step)

        status = self.status()
        add_step("python", True, {
            "executable": sys.executable,
            "version": sys.version.split()[0],
        })

        source_available = bool(status["source"]["available"])
        add_step("source", source_available, status["source"], None if source_available else "SA-LUT source backend is missing")

        torch_state = status["environment"]["torch"]
        torch_available = bool(torch_state.get("available"))
        add_step("torch", torch_available, torch_state, None if torch_available else str(torch_state.get("error") or "PyTorch is unavailable"))

        models = status["models"]
        models_ready = all(model.get("available") for model in models)
        add_step("models", models_ready, models, None if models_ready else "Required model files are missing or Git LFS pointers")

        backend_ok = self._backend_matches(backend, torch_state)
        add_step("backend", backend_ok, {
            "requested": backend,
            "torch": torch_state,
            "forceCpu": self.force_cpu,
        }, None if backend_ok else f"Requested backend is not available: {backend}")

        if capability != "color-transfer":
            add_step("capability", False, {"requested": capability}, "Unsupported smoke test capability")

        can_load = source_available and torch_available and models_ready and backend_ok and capability == "color-transfer"
        if can_load:
            try:
                model = self._load_model({"device": backend})
                add_step("load-model", True, {
                    "selectedDevice": self._device,
                    "deviceInfo": self._device_info,
                    "loadedAt": self._loaded_at,
                })
                try:
                    self._run_tiny_transfer(model)
                    add_step("tiny-input", True, {"shape": [128, 128, 3]})
                except Exception as exc:
                    add_step("tiny-input", False, {"shape": [128, 128, 3]}, str(exc))
            except Exception as exc:
                self._last_error = {
                    "code": "smoke_test_failed",
                    "message": str(exc),
                }
                add_step("load-model", False, None, str(exc))

        passed = all(step["passed"] for step in steps)
        duration_ms = int((time.time() - started) * 1000)
        error_steps = [step for step in steps if not step["passed"]]
        response = {
            "ok": passed,
            "passed": passed,
            "pluginId": PLUGIN_ID,
            "profileId": profile_id,
            "backend": backend,
            "capability": capability,
            "durationMs": duration_ms,
            "environment": {
                "runtime": "python",
                "pythonExecutable": sys.executable,
                "pythonVersion": sys.version.split()[0],
                "torch": torch_state,
                "source": status["source"],
                "selectedDevice": self._device,
                "deviceInfo": self._device_info,
            },
            "models": models,
            "steps": steps,
        }
        if not passed:
            response["error"] = {
                "code": "smoke_test_failed",
                "message": error_steps[0].get("error") or f"Smoke test step failed: {error_steps[0]['name']}",
                "failedSteps": [step["name"] for step in error_steps],
            }
        return (200 if passed else 503), response

    def color_transfer(self, payload: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        task_id = str(payload.get("taskId") or uuid.uuid4())
        payload = dict(payload)
        payload["taskId"] = task_id
        self._set_task_state(
            task_id,
            "queued",
            capability="color-transfer",
            queuePosition=self._task_queue.qsize() + 1,
        )
        self._task_queue.put((task_id, payload))
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

    def _run_color_transfer_task(self, payload: dict[str, Any]) -> tuple[int, dict[str, Any]]:
        task_id = str(payload.get("taskId") or "")
        self._set_task_state(task_id, "running", capability="color-transfer")
        source_path = _input_path(payload, "source", "content")
        style_path = _input_path(payload, "style", "reference")
        output_dir = Path(str(payload.get("outputDir") or os.getcwd()))
        parameters = payload.get("parameters") or {}
        if _bool_parameter(parameters, "mockTask"):
            return self._run_mock_task(task_id, output_dir, parameters)
        output_format = str(parameters.get("outputFormat") or "png").lower()
        if output_format not in ("png", "jpg", "jpeg"):
            output_format = "png"
        ext = "jpg" if output_format == "jpeg" else output_format

        if not source_path or not style_path:
            return self._error(400, task_id, "invalid_input", "inputs.source.path and inputs.style.path are required")
        if not Path(source_path).is_file():
            return self._error(400, task_id, "invalid_input", f"Source image does not exist: {source_path}")
        if not Path(style_path).is_file():
            return self._error(400, task_id, "invalid_input", f"Style image does not exist: {style_path}")

        status = self.status()
        if not status["ready"]:
            return self._error(503, task_id, status.get("reason") or "plugin_not_ready", "SA-LUT is not ready", status)

        output_dir.mkdir(parents=True, exist_ok=True)
        result_path = output_dir / f"{_safe_stem(source_path)}-salut-{task_id[:8]}.{ext}"

        try:
            self._raise_if_cancelled(task_id)
            started = time.time()
            model = self._load_model(parameters, task_id=task_id)
            self._raise_if_cancelled(task_id)
            result = self._run_transfer(task_id, model, source_path, style_path, parameters)
            self._raise_if_cancelled(task_id)
            self._write_image(result_path, result, task_id=task_id)
            elapsed_ms = int((time.time() - started) * 1000)
        except Exception as exc:
            code, domain = _error_domain(exc)
            self._set_task_state(task_id, "cancelled" if code == "TASK_CANCELLED" else "failed", error={"code": code, "domain": domain, "message": str(exc)})
            return self._error(500, task_id, code, str(exc), {"domain": domain})

        output = {
            "id": "result",
            "kind": "image",
            "path": str(result_path),
            "mime": "image/jpeg" if ext == "jpg" else "image/png",
            "sourceInputId": "source",
        }
        self._set_task_state(task_id, "succeeded", outputs=[output], meta={
            "requestedDevice": parameters.get("device", "auto"),
            "selectedDevice": self._device,
            "backend": self._device,
            "elapsedMs": elapsed_ms,
            "model": str(self.ckpt_path),
        }, progress=100, message="Completed")
        return 200, {
            "ok": True,
            "taskId": task_id,
            "status": "succeeded",
            "outputs": [output],
            "meta": {
                "requestedDevice": parameters.get("device", "auto"),
                "selectedDevice": self._device,
                "backend": self._device,
                "elapsedMs": elapsed_ms,
                "model": str(self.ckpt_path),
            },
        }

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
        result_path.write_text(
            f"mock task {task_id} completed in {elapsed_ms} ms\n",
            encoding="utf-8",
        )
        output = {
            "id": "result",
            "kind": "text",
            "path": str(result_path),
            "mime": "text/plain",
        }
        self._set_task_state(task_id, "succeeded", outputs=[output], meta={
            "mock": True,
            "elapsedMs": elapsed_ms,
            "durationMs": duration_ms,
        }, progress=100, message="Completed")
        return 200, {
            "ok": True,
            "taskId": task_id,
            "status": "succeeded",
            "outputs": [output],
            "meta": {
                "mock": True,
                "elapsedMs": elapsed_ms,
                "durationMs": duration_ms,
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
        return 200, {
            "ok": True,
            "taskId": task_id,
            "status": next_status,
        }

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
        return 200, {
            "ok": True,
            "taskId": task_id,
            **state,
        }

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

    def _worker_loop(self) -> None:
        while True:
            task_id, payload = self._task_queue.get()
            try:
                with self._lock:
                    self._active_task_id = task_id
                    cancelled = task_id in self._cancelled_tasks
                    current_status = (self._task_states.get(task_id) or {}).get("status")
                if current_status in TERMINAL_STATUSES:
                    continue
                if cancelled and current_status not in TERMINAL_STATUSES:
                    self._set_task_state(task_id, "cancelled", error={
                        "code": "TASK_CANCELLED",
                        "domain": "task",
                        "message": f"Task cancelled before execution: {task_id}",
                    })
                    continue
                status, response = self._run_color_transfer_task(payload)
                if status >= 400 or response.get("ok") is False:
                    error = dict(response.get("error") or {})
                    code = str(error.get("code") or "TASK_FAILED")
                    if code.upper() == "TASK_CANCELLED":
                        continue
                    if "domain" not in error:
                        error["domain"] = "plugin"
                    self._set_task_state(
                        task_id,
                        "cancelled" if code.upper() == "TASK_CANCELLED" else "failed",
                        error=error,
                    )
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
            import torch  # type: ignore

            cuda_available = bool(torch.cuda.is_available())
            hip_version = getattr(getattr(torch, "version", None), "hip", None)
            device = "cpu" if self.force_cpu or not cuda_available else "cuda"
            gpu_name = torch.cuda.get_device_name(0) if cuda_available else None
            return {
                "available": True,
                "version": getattr(torch, "__version__", None),
                "cudaAvailable": cuda_available,
                "hipVersion": hip_version,
                "rocmAvailable": bool(cuda_available and hip_version),
                "device": device,
                "gpuName": gpu_name,
            }
        except Exception as exc:
            return {
                "available": False,
                "error": str(exc),
            }

    def _backend_matches(self, backend: str, torch_state: dict[str, Any]) -> bool:
        if backend in ("", "auto"):
            return bool(torch_state.get("available"))
        if backend == "cpu":
            return bool(torch_state.get("available"))
        if backend == "cuda":
            return bool(torch_state.get("cudaAvailable") and not torch_state.get("hipVersion"))
        if backend == "rocm":
            return bool(torch_state.get("rocmAvailable") or torch_state.get("hipVersion"))
        if backend == "directml":
            return bool(torch_state.get("directmlAvailable"))
        return False

    def _load_model(self, parameters: dict[str, Any], task_id: str | None = None):
        self._raise_if_cancelled(task_id or "")
        with self._lock:
            model = self._model
        if model is not None:
            if task_id:
                self._set_task_progress(task_id, 18, "SA-LUT model ready")
            return model

        with self._model_lock:
            self._raise_if_cancelled(task_id or "")
            with self._lock:
                model = self._model
            if model is not None:
                if task_id:
                    self._set_task_progress(task_id, 18, "SA-LUT model ready")
                return model

            if task_id:
                self._set_task_progress(task_id, 5, "Resolving SA-LUT backend")
            salut_class, resolve_device, get_device_info = self._load_windows_salut()
            self._raise_if_cancelled(task_id or "")

            requested = str(parameters.get("device") or parameters.get("preferredDevice") or "auto").lower()
            if self.force_cpu:
                requested = "cpu"
            if requested == "rocm":
                requested = "cuda"
            if task_id:
                self._set_task_progress(task_id, 9, f"Selecting SA-LUT device: {requested}")
            device = resolve_device(requested)
            try:
                info = get_device_info(device)
            except Exception:
                info = {"device": str(device), "backend": "unknown", "name": "Unknown"}
            self._raise_if_cancelled(task_id or "")
            if task_id:
                self._set_task_progress(task_id, 12, "Loading SA-LUT model")
            model = salut_class(
                ckpt_path=str(self.ckpt_path),
                vgg_path=str(self.vgg_path) if self.vgg_path.exists() else "",
                device=device,
            )
            with self._lock:
                self._model = model
                self._device = str(getattr(model, "device", device))
                self._device_info = info
                self._loaded_at = time.time()
            self._raise_if_cancelled(task_id or "")
            if task_id:
                self._set_task_progress(task_id, 18, "SA-LUT model ready")
            return model

    def _load_windows_salut(self):
        if not self.source_root.exists():
            raise RuntimeError(f"Windows SA-LUT backend does not exist: {self.source_root}")
        if str(self.source_root) not in sys.path:
            sys.path.insert(0, str(self.source_root))
        from engine.device_manager import get_device_info, resolve_device  # type: ignore
        try:
            from engine.salut import SALUTInference  # type: ignore
        except ImportError:
            from engine.salut.inference import SALUTInference  # type: ignore

        return SALUTInference, resolve_device, get_device_info

    def _run_transfer(self, task_id: str, model: Any, source_path: str, style_path: str, parameters: dict[str, Any]):
        self._raise_if_cancelled(task_id)
        self._set_task_progress(task_id, 25, "Reading source image")
        source = self._read_image(source_path)
        self._raise_if_cancelled(task_id)
        self._set_task_progress(task_id, 35, "Reading style image")
        style = self._read_image(style_path)
        if source is None:
            raise RuntimeError(f"Failed to read source image: {source_path}")
        if style is None:
            raise RuntimeError(f"Failed to read style image: {style_path}")
        self._raise_if_cancelled(task_id)
        self._set_task_progress(task_id, 45, "Preparing SA-LUT inference")
        analysis_size = _int_parameter(parameters, "analysisSize", 1024, 128, 2048)
        self._raise_if_cancelled(task_id)
        self._set_task_progress(task_id, 50, "Running SA-LUT inference")
        result = model.transfer(source, style, analysis_size=analysis_size)
        self._raise_if_cancelled(task_id)
        self._set_task_progress(task_id, 85, "SA-LUT inference completed")
        return result

    def _run_tiny_transfer(self, model: Any) -> None:
        import numpy as np  # type: ignore

        source = np.zeros((128, 128, 3), dtype=np.uint8)
        style = np.full((128, 128, 3), 128, dtype=np.uint8)
        result = model.transfer(source, style, analysis_size=128)
        if result is None:
            raise RuntimeError("Tiny transfer returned no result")
        shape = getattr(result, "shape", None)
        if not shape or len(shape) < 2:
            raise RuntimeError("Tiny transfer returned an invalid result")

    def _read_image(self, path: str):
        import cv2  # type: ignore
        import numpy as np  # type: ignore

        image_path = Path(path)
        if image_path.suffix.lower() in RAW_IMAGE_EXTENSIONS:
            try:
                import rawpy  # type: ignore

                with rawpy.imread(str(image_path)) as raw:
                    rgb = raw.postprocess(
                        use_camera_wb=True,
                        no_auto_bright=False,
                        output_bps=8,
                    )
                return cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
            except Exception as exc:
                raise RuntimeError(f"Failed to decode RAW image '{path}': {exc}") from exc

        try:
            data = np.fromfile(path, dtype=np.uint8)
        except OSError:
            return None
        if data.size == 0:
            return None
        return cv2.imdecode(data, cv2.IMREAD_COLOR)

    def _write_image(self, path: Path, image: Any, task_id: str | None = None) -> None:
        import cv2  # type: ignore

        self._raise_if_cancelled(task_id or "")
        if task_id:
            self._set_task_progress(task_id, 90, "Encoding output image")
        ext = path.suffix or ".png"
        ok, data = cv2.imencode(ext, image)
        if not ok:
            raise RuntimeError(f"Failed to write output image: {path}")
        self._raise_if_cancelled(task_id or "")
        if task_id:
            self._set_task_progress(task_id, 94, "Writing output image")
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp_path = path.with_name(f"{path.name}.{uuid.uuid4().hex}.tmp")
        try:
            with tmp_path.open("wb") as handle:
                handle.write(data.tobytes())
                handle.flush()
                os.fsync(handle.fileno())
            self._raise_if_cancelled(task_id or "")
            if task_id:
                self._set_task_progress(task_id, 98, "Finalizing output image")
            os.replace(tmp_path, path)
        finally:
            if tmp_path.exists():
                try:
                    tmp_path.unlink()
                except OSError:
                    pass

    def _error(self, status: int, task_id: str, code: str, message: str, details: Any | None = None) -> tuple[int, dict[str, Any]]:
        error = {
            "code": code,
            "message": message,
        }
        if isinstance(details, dict) and details.get("domain"):
            error["domain"] = details["domain"]
            extra = {key: value for key, value in details.items() if key != "domain"}
            if extra:
                error["details"] = extra
        elif details is not None:
            error["details"] = details
        self._last_error = error
        return status, {
            "ok": False,
            "taskId": task_id,
            "error": error,
        }



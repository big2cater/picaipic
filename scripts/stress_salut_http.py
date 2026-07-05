from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PLUGIN_ROOT = ROOT / "plugins" / "picai-salut-color"
BACKEND_MAIN = PLUGIN_ROOT / "backend" / "main.py"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def request_json(method: str, url: str, payload: dict[str, Any] | None = None, timeout: float = 5) -> tuple[int, dict[str, Any]]:
    data = None
    headers = {"Accept": "application/json"}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json; charset=utf-8"
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read().decode("utf-8")
            return int(response.status), json.loads(body) if body else {}
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8")
        return int(error.code), json.loads(body) if body else {}


def wait_health(base_url: str, timeout_s: float) -> None:
    deadline = time.monotonic() + timeout_s
    last_error = ""
    while time.monotonic() < deadline:
        try:
            status, payload = request_json("GET", f"{base_url}/health", timeout=1)
            if status == 200 and payload.get("pluginId") == "picai-salut-color":
                return
        except Exception as exc:
            last_error = str(exc)
        time.sleep(0.1)
    raise TimeoutError(f"Plugin HTTP server did not become healthy: {last_error}")


def wait_terminal(base_url: str, task_id: str, timeout_s: float) -> dict[str, Any]:
    cursor = 0
    deadline = time.monotonic() + timeout_s
    last: dict[str, Any] = {}
    all_events: list[dict[str, Any]] = []
    while time.monotonic() < deadline:
        query = urllib.parse.urlencode({"after": cursor, "timeoutMs": 1000})
        status, payload = request_json("GET", f"{base_url}/tasks/{task_id}/events?{query}", timeout=3)
        if status != 200:
            raise RuntimeError(f"Task events failed for {task_id}: {status} {payload}")
        last = payload
        all_events.extend(payload.get("events") or [])
        cursor = int(payload.get("nextCursor") or cursor)
        state = payload.get("state") or {}
        if payload.get("terminal") or state.get("status") in {"succeeded", "failed", "cancelled"}:
            payload = dict(payload)
            payload["events"] = all_events
            return payload
    raise TimeoutError(f"Task did not finish: {task_id}; last={last}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Stress PicAiPic SA-LUT async HTTP protocol with mock work.")
    parser.add_argument("--tasks", type=int, default=8)
    parser.add_argument("--duration-ms", type=int, default=500)
    parser.add_argument("--cancel-every", type=int, default=3)
    parser.add_argument("--timeout-s", type=float, default=20)
    args = parser.parse_args()

    port = free_port()
    base_url = f"http://127.0.0.1:{port}"
    env = os.environ.copy()
    env["PICAIPIC_PLUGIN_PORT"] = str(port)
    env["PICAIPIC_PLUGIN_ROOT"] = str(PLUGIN_ROOT)

    process = subprocess.Popen(
        [sys.executable, str(BACKEND_MAIN)],
        cwd=str(PLUGIN_ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        wait_health(base_url, args.timeout_s)
        output_dir = PLUGIN_ROOT / "tmp" / "http-stress"
        output_dir.mkdir(parents=True, exist_ok=True)

        task_ids: list[str] = []
        for index in range(args.tasks):
            task_id = f"http-stress-{int(time.time() * 1000)}-{index}"
            status, payload = request_json("POST", f"{base_url}/invoke/color-transfer", {
                "taskId": task_id,
                "inputs": {},
                "parameters": {
                    "mockTask": True,
                    "mockDurationMs": args.duration_ms,
                    "mockStepMs": 50,
                },
                "outputDir": str(output_dir),
            })
            if status != 202 or payload.get("status") != "queued":
                raise RuntimeError(f"Unexpected invoke response for {task_id}: {status} {payload}")
            task_ids.append(task_id)

        cancelled = set()
        if args.cancel_every > 0:
            for index, task_id in enumerate(task_ids):
                if (index + 1) % args.cancel_every == 0:
                    status, payload = request_json("POST", f"{base_url}/tasks/{task_id}/cancel", {"taskId": task_id})
                    if status != 200 or payload.get("status") not in {"cancelled", "cancelling"}:
                        raise RuntimeError(f"Unexpected cancel response for {task_id}: {status} {payload}")
                    cancelled.add(task_id)

        results = {}
        for task_id in task_ids:
            payload = wait_terminal(base_url, task_id, args.timeout_s)
            state = payload.get("state") or {}
            results[task_id] = {
                "status": state.get("status"),
                "events": [event.get("status") for event in payload.get("events") or []],
                "progressEvents": [event.get("progress") for event in payload.get("events") or [] if event.get("type") == "task.progress"],
                "outputs": state.get("outputs") or [],
            }

        counts: dict[str, int] = {}
        for result in results.values():
            status = str(result["status"])
            counts[status] = counts.get(status, 0) + 1

        expected_cancelled = len(cancelled)
        if counts.get("cancelled", 0) != expected_cancelled:
            raise AssertionError(f"Expected {expected_cancelled} cancelled tasks, got {counts}")
        if counts.get("succeeded", 0) != args.tasks - expected_cancelled:
            raise AssertionError(f"Expected remaining tasks to succeed, got {counts}")
        if not any(result["progressEvents"] for result in results.values() if result["status"] == "succeeded"):
            raise AssertionError("Expected at least one progress event for a succeeded task")

        running_id = f"http-running-cancel-{int(time.time() * 1000)}"
        request_json("POST", f"{base_url}/invoke/color-transfer", {
            "taskId": running_id,
            "inputs": {},
            "parameters": {
                "mockTask": True,
                "mockDurationMs": max(args.duration_ms, 1000),
                "mockStepMs": 50,
            },
            "outputDir": str(output_dir),
        })
        deadline = time.monotonic() + args.timeout_s
        while time.monotonic() < deadline:
            status, payload = request_json("GET", f"{base_url}/tasks/{running_id}", timeout=2)
            if status == 200 and payload.get("status") == "running":
                break
            time.sleep(0.02)
        _status, snapshot = request_json("GET", f"{base_url}/tasks/{running_id}/events?after=0&timeoutMs=0", timeout=2)
        cursor = int(snapshot.get("nextCursor") or 0)
        observed: dict[str, Any] = {}

        def observe_cancel() -> None:
            query = urllib.parse.urlencode({"after": cursor, "timeoutMs": 10000})
            _event_status, event_payload = request_json("GET", f"{base_url}/tasks/{running_id}/events?{query}", timeout=12)
            observed.update(event_payload)

        observer = threading.Thread(target=observe_cancel, daemon=True)
        observer.start()
        time.sleep(0.1)
        request_json("POST", f"{base_url}/tasks/{running_id}/cancel", {"taskId": running_id})
        observer.join(timeout=12)
        if not observed.get("events"):
            raise AssertionError(f"Long-poll observer did not receive cancel event: {observed}")
        running_payload = wait_terminal(base_url, running_id, args.timeout_s)
        running_state = running_payload.get("state") or {}
        if running_state.get("status") != "cancelled":
            raise AssertionError(f"Expected running HTTP cancellation, got {running_state}")

        print(json.dumps({
            "ok": True,
            "baseUrl": base_url,
            "tasks": args.tasks,
            "counts": counts,
            "runningCancel": {
                "taskId": running_id,
                "status": running_state.get("status"),
                "events": [event.get("status") for event in running_payload.get("events") or []],
            },
            "results": results,
        }, ensure_ascii=False, indent=2))
        return 0
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())

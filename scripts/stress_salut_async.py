from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BACKEND = ROOT / "plugins" / "picai-salut-color" / "backend"
if str(BACKEND) not in sys.path:
    sys.path.insert(0, str(BACKEND))

from salut_adapter import SalutAdapter  # noqa: E402


def wait_terminal(adapter: SalutAdapter, task_id: str, timeout_s: float) -> dict:
    cursor = 0
    deadline = time.monotonic() + timeout_s
    last = {}
    all_events = []
    while time.monotonic() < deadline:
        _status, payload = adapter.task_events(task_id, after=cursor, timeout_ms=1000)
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
    parser = argparse.ArgumentParser(description="Stress PicAiPic SA-LUT async task plumbing with mock work.")
    parser.add_argument("--tasks", type=int, default=8)
    parser.add_argument("--duration-ms", type=int, default=500)
    parser.add_argument("--cancel-every", type=int, default=3)
    parser.add_argument("--timeout-s", type=float, default=20)
    args = parser.parse_args()

    adapter = SalutAdapter()
    output_dir = ROOT / "plugins" / "picai-salut-color" / "tmp" / "async-stress"
    output_dir.mkdir(parents=True, exist_ok=True)

    task_ids: list[str] = []
    for index in range(args.tasks):
        task_id = f"stress-{int(time.time() * 1000)}-{index}"
        status, response = adapter.color_transfer({
            "taskId": task_id,
            "inputs": {},
            "parameters": {
                "mockTask": True,
                "mockDurationMs": args.duration_ms,
                "mockStepMs": 50,
            },
            "outputDir": str(output_dir),
        })
        if status != 202 or response.get("status") != "queued":
            raise RuntimeError(f"Unexpected invoke response for {task_id}: {status} {response}")
        task_ids.append(task_id)

    cancelled = set()
    if args.cancel_every > 0:
        for index, task_id in enumerate(task_ids):
            if (index + 1) % args.cancel_every == 0:
                adapter.cancel_task(task_id)
                cancelled.add(task_id)

    results = {}
    for task_id in task_ids:
        payload = wait_terminal(adapter, task_id, args.timeout_s)
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

    running_id = f"stress-running-cancel-{int(time.time() * 1000)}"
    adapter.color_transfer({
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
        _status, status_payload = adapter.task_status(running_id)
        if status_payload.get("status") == "running":
            break
        time.sleep(0.02)
    adapter.cancel_task(running_id)
    running_cancel = wait_terminal(adapter, running_id, args.timeout_s)
    running_state = running_cancel.get("state") or {}
    if running_state.get("status") != "cancelled":
        raise AssertionError(f"Expected running task cancellation, got {running_state}")

    print(json.dumps({
        "ok": True,
        "tasks": args.tasks,
        "counts": counts,
        "runningCancel": {
            "taskId": running_id,
            "status": running_state.get("status"),
            "events": [event.get("status") for event in running_cancel.get("events") or []],
        },
        "results": results,
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

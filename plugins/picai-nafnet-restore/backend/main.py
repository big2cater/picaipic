from __future__ import annotations

import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

from nafnet_adapter import NAFNetAdapter


PLUGIN_ID = "picai-nafnet-restore"
PORT = int(os.environ.get("PICAIPIC_PLUGIN_PORT", "8012"))
AUTH_TOKEN = os.environ.get("PICAIPIC_PLUGIN_AUTH_TOKEN", "")
ADAPTER = NAFNetAdapter()


def check_auth(handler: BaseHTTPRequestHandler) -> bool:
    """Return True if the request is authorized (or token is not configured)."""
    if not AUTH_TOKEN:
        return True
    auth = handler.headers.get("Authorization", "")
    return auth == f"Bearer {AUTH_TOKEN}"


def json_response(handler: BaseHTTPRequestHandler, status: int, payload: dict) -> None:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json; charset=utf-8")
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def read_json(handler: BaseHTTPRequestHandler) -> dict:
    length = int(handler.headers.get("Content-Length", "0"))
    if length <= 0:
        return {}
    raw = handler.rfile.read(length)
    return json.loads(raw.decode("utf-8"))


class Handler(BaseHTTPRequestHandler):
    server_version = "PicAiPicNAFNet/0.1"

    def log_message(self, fmt: str, *args) -> None:
        log_dir = Path(
            os.environ.get("PICAIPIC_PLUGIN_LOG_DIR")
            or Path(os.environ.get("PICAIPIC_PLUGIN_DATA_DIR", ".")).joinpath("logs")
        )
        log_dir.mkdir(parents=True, exist_ok=True)
        with log_dir.joinpath("plugin.log").open("a", encoding="utf-8") as log:
            log.write(fmt % args)
            log.write("\n")

    def do_GET(self) -> None:
        path = urlparse(self.path).path
        if path == "/health":
            status = ADAPTER.status()
            json_response(self, 200, {
                "pluginId": PLUGIN_ID,
                "ready": True,
                "inferenceReady": status["ready"],
                "version": "0.1.0",
            })
            return

        if not check_auth(self):
            json_response(self, 401, {"error": "unauthorized"})
            return

        if path == "/status":
            json_response(self, 200, ADAPTER.status())
            return

        if path == "/diagnostics":
            json_response(self, 200, ADAPTER.diagnostics())
            return

        if path.startswith("/tasks/"):
            parts = [part for part in path.split("/") if part]
            if len(parts) == 3 and parts[2] == "events":
                query = parse_qs(urlparse(self.path).query)
                try:
                    after = int((query.get("after") or ["0"])[0])
                except (TypeError, ValueError):
                    after = 0
                try:
                    timeout_ms = int((query.get("timeoutMs") or ["25000"])[0])
                except (TypeError, ValueError):
                    timeout_ms = 25000
                status, response = ADAPTER.task_events(parts[1], after=after, timeout_ms=timeout_ms)
                json_response(self, status, response)
                return
            if len(parts) == 2:
                status, response = ADAPTER.task_status(parts[1])
                json_response(self, status, response)
                return

        json_response(self, 404, {
            "ok": False,
            "error": {
                "code": "not_found",
                "message": f"Unknown endpoint: {path}",
            },
        })

    def do_POST(self) -> None:
        path = urlparse(self.path).path
        if not check_auth(self):
            json_response(self, 401, {"error": "unauthorized"})
            return
        try:
            payload = read_json(self)
        except Exception as exc:
            json_response(self, 400, {
                "ok": False,
                "error": {
                    "code": "invalid_json",
                    "message": str(exc),
                },
            })
            return

        if path in ("/smoke-test", "/verify"):
            status, response = ADAPTER.smoke_test(payload)
            json_response(self, status, response)
            return

        if path.startswith("/invoke/"):
            capability = path.removeprefix("/invoke/")
            status, response = ADAPTER.invoke(capability, payload)
            json_response(self, status, response)
            return

        if path.startswith("/tasks/") and path.endswith("/cancel"):
            parts = [part for part in path.split("/") if part]
            task_id = parts[1] if len(parts) >= 2 else str(payload.get("taskId") or "")
            status, response = ADAPTER.cancel_task(task_id)
            json_response(self, status, response)
            return

        json_response(self, 404, {
            "ok": False,
            "error": {
                "code": "not_found",
                "message": f"Unknown endpoint: {path}",
            },
        })


def main() -> None:
    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    print(f"{PLUGIN_ID} listening on 127.0.0.1:{PORT}", flush=True)
    print(f"python: {os.sys.executable}", flush=True)
    print(f"nafnet source: {ADAPTER.source_root}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()

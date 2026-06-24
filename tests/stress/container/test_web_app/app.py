#!/usr/bin/env python3
"""Mock target application for Riku container traffic tests.

Deliberately stdlib-only (no FastAPI/uvicorn) to avoid pip-install flake
inside the test container's plugins/python build step (which only runs
`pip install -r requirements.txt` — an empty requirements.txt is enough
to satisfy the plugin's detect() check, see plugins/python).

Riku injects $PORT for `web` process types (src/deploy/workers.rs:212)
and proxies external port 80 to it via nginx — this app must bind
0.0.0.0:$PORT, which it does below.
"""
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STATE_PATH = os.path.join(os.path.dirname(__file__), "state.json")
DEPLOY_ENV = os.environ.get("DEPLOY_ENV", "unset")
START_TIME = time.time()

_state_lock = threading.Lock()


def load_state():
    with _state_lock:
        with open(STATE_PATH, "r") as f:
            return json.load(f)


class Handler(BaseHTTPRequestHandler):
    server_version = "RikuTestApp/1.0"

    def _send_json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            self._send_json(200, {"status": "ok", "uptime_s": round(time.time() - START_TIME, 1)})
            return

        if self.path == "/":
            try:
                state = load_state()
            except Exception as e:
                self._send_json(500, {"error": str(e)})
                return
            self._send_json(200, {
                "deploy_env": DEPLOY_ENV,
                "pid": os.getpid(),
                "uptime_s": round(time.time() - START_TIME, 1),
                "state": state,
            })
            return

        self._send_json(404, {"error": "not found", "path": self.path})

    def log_message(self, fmt, *args):
        # Quiet by default; uncomment for verbose request logging.
        pass


def main():
    port = int(os.environ.get("PORT", "8080"))
    server = ThreadingHTTPServer(("0.0.0.0", port), Handler)
    print(f"[test_web_app] listening on 0.0.0.0:{port} deploy_env={DEPLOY_ENV}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()

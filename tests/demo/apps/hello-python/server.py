#!/usr/bin/env python3
"""riku sandbox demo app (Python runtime) -- also the one with the
sqlite-volume addon bound to it, so DATABASE_PATH shows up in its env."""
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(os.environ.get("PORT", "8080"))


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        db_path = os.environ.get("SQLITE_PATH", "(no addon bound)")
        body = f"hello from hello-python (pid {os.getpid()})\nSQLITE_PATH={db_path}\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, fmt, *args):
        pass


if __name__ == "__main__":
    HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()

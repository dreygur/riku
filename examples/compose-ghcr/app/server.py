#!/usr/bin/env python3
"""Tiny stdlib-only HTTP server for the riku compose/GHCR demo.

Prints VERSION (baked in at image build time) and the current time, so a
rebuilt-and-pushed image is visibly different from the one it replaces --
that's the whole point of this demo: watch the response change after
`riku`'s image-watch check picks up a new push, with no `git push riku`.
"""
import datetime
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

VERSION = os.environ.get("VERSION", "unset")
PORT = int(os.environ.get("PORT", "8080"))


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = f"version={VERSION} time={datetime.datetime.now(datetime.UTC).isoformat()}\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, fmt, *args):
        pass  # keep worker logs quiet; riku captures stdout/stderr anyway


if __name__ == "__main__":
    HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()

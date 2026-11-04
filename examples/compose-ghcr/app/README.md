# app

The image for the [compose-ghcr demo](../README.md): a stdlib-only Python
HTTP server that answers every `GET` with `version=<VERSION> time=<now>`, so a
rebuilt and pushed image is visibly different from the one it replaces.

- `Dockerfile`: `python:3.12-alpine`, bakes the `VERSION` build arg into the image, exposes 8080.
- `server.py`: `Handler` (`do_GET`, and a silenced `log_message`); listens on `0.0.0.0` at `PORT`, 8080 when unset.

## Verification
- Run: `docker build --build-arg VERSION=v1 -t riku-compose-demo . && docker run --rm -p 8080:8080 riku-compose-demo`, then `curl http://127.0.0.1:8080`

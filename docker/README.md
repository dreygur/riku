# docker

The container image for the shell e2e suite. CI builds and runs it
(`.github/workflows/ci.yml`), and `scripts/run-docker-tests.sh` does the same
locally.

- `Dockerfile.test`: a Debian bookworm image with nginx, Node, Python and Rust that builds riku from the repo and installs it to `/usr/local/bin/riku`.
- `test-entrypoint.sh`: starts nginx and `riku router-helper` as root (nginx loads `/etc/riku/nginx/*.conf`), then runs every `tests/e2e/cases/[0-9]*.sh` in order through `run_test`, and exits non-zero if any case failed.

## Invariants
- [test-entrypoint.sh] Runs under `set -euo pipefail`
- [test-entrypoint.sh] Each case gets its own temp `RIKU_ROOT`. nginx sites are the helper's, in `/etc/riku/nginx`, and each case destroys or leaves its own app

## Verification
- Run: `docker build -f docker/Dockerfile.test -t riku-e2e:ci . && docker run --rm riku-e2e:ci`

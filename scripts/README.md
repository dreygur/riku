# scripts

Developer and install scripts.

- `install.sh`: the one-line installer. Detects OS and arch, downloads the matching release tarball with curl or wget, checks it against the release `.sha256` when one is published, and installs `riku` (or `rk` with `--rk`). `RIKU_VERSION`, `RIKU_INSTALL_DIR` and `RIKU_NO_INIT=1` override the tag, the directory and the next-step hint. Helpers: `say`, `err`, `have`.
- `coverage.sh`: installs `cargo-tarpaulin` if missing and writes an HTML coverage report to `coverage/`.
- `run-docker-tests.sh`: builds `docker/Dockerfile.test` and runs the e2e suite in it, with docker or podman.
- `dashboard-tunnel.sh`: runs the embedded dashboard API on a server as the deploy user and forwards it to this machine over `ssh -L` for as long as the terminal stays open. Log in as a user who can sudo. The token is passed as an argument there, so other users on the server can see it in `ps` while it runs.
- `dashboard-web.sh`: builds and runs the Next.js dashboard (`dashboard/`) on `127.0.0.1` against the API the tunnel opens. Needs nub.
- [lib](./lib/README.md): shared shell helpers.

## Invariants
- [install.sh] Runs under `set -eu`, and an `EXIT` trap removes its temp directory
- [coverage.sh, run-docker-tests.sh, dashboard-tunnel.sh, dashboard-web.sh] Run under `set -euo pipefail`

## Verification
- Run: `shellcheck scripts/*.sh scripts/lib/*.sh`

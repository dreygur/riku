# scripts

Developer and install scripts.

- `install.sh`: the one-line installer. `pick_platform` names the release build from the OS, CPU (`armv6l` is armv6; a 64 bit kernel with a 32 bit userland gets armv7) and C library (glibc 2.31 or newer gets the glibc build, anything else the static musl one). `main` downloads that tarball with curl or wget, checks it against its `.sha256` when one is published, and installs `riku` (or `rk` with `--rk`). `RIKU_VERSION`, `RIKU_INSTALL_DIR`, `RIKU_LIBC` (`gnu` or `musl`) and `RIKU_NO_INIT=1` override the tag, the directory, the C library and the next-step hint. With `RIKU_INSTALL_LIB=1` the script only defines its functions; `tests/install/pick_platform.sh` uses that.
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

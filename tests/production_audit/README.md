# Production audit suite for the Riku supervisor

Targets `src/supervisor/` behavior under stress, not feature correctness.
All scripts drive the real CLI (`riku ps`, `riku config set`, `riku apps`,
`riku supervisor`) — no internal APIs are invented.

Known facts baked into these scripts (verified against source before
writing, not assumed):

- No `SIGCHLD` handler exists anywhere in `src/supervisor/`. Crash
  detection and reaping are poll-based via `try_wait()` inside
  `check_processes()` (`src/supervisor/process/health_check.rs`), invoked
  from the 1s `recv_timeout` tick in `src/supervisor/daemon/mod.rs`.
- Resource limits (`src/supervisor/resource_limits/mod.rs`) are loaded
  once via `ResourceLimits::from_env()` at supervisor startup and applied
  identically to every spawned worker. There is **no per-app TOML field**
  for this — `riku.toml` in `bad_tenant_app/` is a documentation-only
  artifact, not something Riku parses. The real lever is the env vars in
  `bad_tenant_app/start-supervisor.env`.
- No cgroups anywhere in `src/`. `RLIMIT_CPU` is a total-consumption cap,
  not a fair-share scheduler — a CPU-spin tenant can peg one core until
  the limit is hit.

## Files

- `stress_lifecycle.sh` — 100x scale 1->20->1, watches for zombie (`Z`
  state) descendants of the supervisor pid.
- `leak_monitor.sh` — 50x `riku config set` + forced `SIGHUP` reload,
  tracks `/proc/<supervisor_pid>/fd` count and RSS over the run.
- `chaos_signals.sh` — finds a worker's real OS pid, `kill -9`s it
  directly, measures actual detect+respawn latency (not assumed to be
  sub-2s — the code has exponential backoff + jitter).
- `resource_limit_audit.sh` — runs `bad_tenant_app` under the env-var
  limits and checks whether it's actually bounded, and by what (SIGXCPU
  vs OOM-killer vs nothing).
- `bad_tenant_app/` — hostile test tenant (`app.py mem` / `app.py cpu`),
  `Procfile`, `riku.toml` (intent-only, see warnings in the file),
  `start-supervisor.env` (the real, functional limit mechanism).
- `run_all.sh` — runs everything above in order, writes a summary log.
- `container/` — full containerized integration suite: builds a real
  target server image (sshd, nginx, the compiled `riku` binary, bundled
  runtime plugins), deploys a mock app via real `git push` over SSH, and
  drives concurrent HTTP load against it. See `container/` below.

## Containerized integration suite (`container/`)

Verifies the full real-world path end to end: SSH-gated git push deploy
→ runtime plugin detection/build → supervisor spawn → nginx vhost
generation/reload → live HTTP traffic — against an actual container, not
mocks. Works with either Docker or Podman (auto-detected).

```bash
./tests/production_audit/container/run_container_test.sh
```

**Latest verified run (2026-06-18): PASS.** 14,530/14,530 requests
succeeded (0 502/504s) under 80 concurrent workers for 30s; supervisor
stayed alive; zero zombie processes. Latency: p50 19.7ms, p95 423.6ms,
p99 1236.7ms.

Files:
- `container/Dockerfile` — Ubuntu 24.04 target server image.
- `container/entrypoint.sh` — imports bootstrap SSH key, runs
  `riku init --no-systemd`, starts sshd/nginx/`riku supervisor`.
- `container/sudoers-riku-nginx`, `container/nginx-wrapper.sh` — scoped
  passwordless-sudo path letting the unprivileged deploy user reload the
  root-owned nginx master (see `ARCHITECTURE.md` Security Model).
- `container/test_web_app/` — mock app deployed during the test.
- `container/user_traffic_simulation.sh` — host-side git push + load test
  (prefers `wrk`/`k6`, falls back to a parallel curl loop).
- `container/run_container_test.sh` — orchestrator: build binary → build
  image → provision SSH keypair → run container → drive traffic →
  collect logs → verdict.

Getting this suite green found one real product bug, since fixed: the
first-push bare-repo init bug in `src/cli/git/receive_pack.rs` (see the git
history for that fix).

## Running (non-containerized suite)

```bash
# normal lifecycle/leak/chaos tests
riku supervisor &
./tests/production_audit/run_all.sh

# resource-limit tests need the supervisor started with the bad-tenant
# ceilings active instead:
set -a; source ./tests/production_audit/bad_tenant_app/start-supervisor.env; set +a
riku supervisor &
./tests/production_audit/resource_limit_audit.sh badtenant mem
./tests/production_audit/resource_limit_audit.sh badtenant cpu
```

Results land in `tests/production_audit/results/` (created on first run,
gitignored is your call — not added here).

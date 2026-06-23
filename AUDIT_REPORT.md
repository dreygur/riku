# Riku — Comprehensive Repository Audit

_Staff-engineer / security / architecture / QA / OSS-maintainer review._
_Date: 2026-06-23. Branch: `feat/dashboard-control-plane`. Scope: full repo (~20.5k Rust LOC + Next.js dashboard + shell plugins)._

Evidence is cited as `file:line`. Findings are tagged **Verified** (traced in code), **Likely** (strong inference), or **Suspected** (needs runtime confirmation).

---

## Phase 1 — Repository Discovery

**Technology stack**
- **Core:** Rust 2021, single binary. clap 4 (CLI), axum 0.7.9 + tokio 1.49 (health/control HTTP), tera 1.20 (nginx templates), nix 0.29 + libc (namespaces, signals, cgroups), serde/serde_json, anyhow + thiserror, chrono, regex, once_cell, reqwest 0.12.
- **Dashboard:** Next.js 16, React 19, Hono 4 (API router), Tailwind v4, Playwright. Runs as a separate Node service proxying to the Rust supervisor.
- **Runtime plugins:** shell scripts (`plugins/node|python|ruby|go|rust-lang`) + 3 Rust binary plugin crates (java, clojure, container).

**Architecture** — layered as documented in CLAUDE.md/ARCHITECTURE.md: `cli/` + `plugins/` (provider) → `deploy/` + `supervisor/` + `nginx/` (service) → `config/` (repository). Layering is largely respected.

**Entry points / critical paths**
- `src/main.rs` → `cli/cli.rs` (269 LOC) clap dispatch.
- **Deploy path:** SSH `git push` → `cli/git/receive_pack.rs` → post-receive hook `cli/git/hook.rs:cmd_git_hook` → `deploy::do_deploy` → runtime plugin `detect/build/env/start` (`plugins/runtime.rs`, `plugins/executor.rs`) → worker TOML written → supervisor picks up.
- **Supervisor loop:** `supervisor/daemon/mod.rs:run` (single-threaded event loop) drives reload-on-SIGHUP, health checks, log rotation, cron, stats.
- **Control plane:** `supervisor/health/` axum server on `127.0.0.1:<port>` — read-only `/health` `/metrics*` (open) + mutating `/control/*` (bearer-token gated).
- **Dashboard:** browser → Next.js `/api/*` (Hono) → Rust control plane with server-attached token.

**Build:** cargo workspace (root + 3 plugin crates). CI (`.github/workflows/ci.yml`): fmt, `clippy -D warnings`, build, test, release build, shell deploy tests, `cargo audit --deny warnings`, `cargo deny check`. Solid gate.

**Overall verdict:** This is a **mature, security-conscious codebase**, not a prototype. Most "audit red flags" (unwraps, panics) are confined to tests. The findings below are real but the baseline quality is high.

---

## Phase 2 — Completion Analysis

| Feature | Status | Confidence | Evidence |
|---|---|---|---|
| git-push deploy (SSH) | Complete | Verified | `cli/git/*`, `deploy/`, container integration test PASS (PROJECT_STATUS.md) |
| Runtime plugin protocol (detect/build/env/start) | Complete | Verified | `plugins/runtime.rs`, `executor.rs`, bundled `plugins/*` |
| Process supervisor (spawn/health/restart/stop) | Complete | Verified | `supervisor/process/*`, `daemon/mod.rs` |
| Namespace isolation (mnt/net/pid) + cgroups | Complete | Verified | `process/isolation.rs`, `cgroups/`, `__ns-shim` subcommand |
| Resource limits | Complete | Verified | `supervisor/resource_limits/` |
| Log rotation | Complete | Verified | `supervisor/log_rotation/`, hardened vs external rotation (commit aa88540) |
| nginx config generation + SSL/ACME | Complete | Verified | `nginx/`, `templates/*.tera` |
| Cron scheduler | **Partially Complete (buggy)** | Verified | wired at `daemon/mod.rs:257`; **day/weekday logic bug** + Procfile-vs-scheduler grammar mismatch (Phase 3) |
| Control-plane HTTP API + token auth | Complete | Verified | `health/control.rs`, `health/auth.rs` |
| Dashboard (Next.js) | **Partially Complete** | Verified | functional but **no authentication layer** (Phase 4) |
| Container build/export + remote deploy | Complete | Verified | `deploy/container_runtime.rs`, `crates/riku-plugin-container` |
| Typed error domain (`error.rs`) | **Partially adopted** | Verified | used in 4 sites; most code still `anyhow`; many variants unused (`#![allow(dead_code)]`) |

**Completion estimates:** Overall **~85%**. Backend/CLI **~90%**. Supervisor **~88%** (cron bug). Dashboard **~70%** (no auth). Infra/CI **~90%**. Testing **~80%**. Docs **~85%** (extensive: README, ARCHITECTURE, API, mkdocs site, PROJECT_STATUS).

---

## Phase 3 — Bug Audit

### B1 — Cron day-of-month / day-of-week is wrong **[Verified, High]**
`src/supervisor/cron/parser.rs:106-110`:
```rust
&& (day_parts.contains(&day) || weekday_parts.contains(&weekday))
```
A `*` day-of-month field expands to the full set `1..=31` (`parse_cron_field(parts[2],1,31)`), so `day_parts.contains(&day)` is **always true**, and the `||` makes the weekday clause dead. **Result: `0 9 * * 1` ("9am every Monday") fires every day at 09:00.** Standard cron semantics: OR the two fields **only when both are restricted**; otherwise AND. 
**Fix:** track whether each field was `*`; apply OR only if both day and weekday are non-`*`, else AND.

### B2 — Procfile cron grammar rejects valid cron the scheduler supports **[Verified, Medium]**
`src/util/procfile.rs:15` `CRON_REGEXP` accepts only single int, `*`, or `*/N` per field. But `cron/parser.rs:parse_cron_field` supports ranges (`1-5`), lists (`1,3,5`), and `a-b/s`. So a Procfile line `cron: 0 9 * * 1-5 cmd` is **rejected at parse time** even though the engine would run it. Two divergent cron grammars. 
**Fix:** validate Procfile cron via `cron::validate_cron_expression`, delete the regex.

### B3 — Cron runs in UTC, not local time **[Verified, Low/Medium]**
`parser.rs:88` uses `.naive_utc()`. `0 0 * * *` runs at 00:00 UTC regardless of server TZ. Surprising for "daily at midnight"; undocumented. 
**Fix:** document, or use `chrono::Local`.

### B4 — Cron impossible-date silent fallback **[Verified, Low]**
`parser.rs:116-118`: if no match within a 1-year scan (e.g. `* * 30 2 *` — Feb 30), it silently returns `now + 1h`, then repeats hourly forever instead of erroring. 
**Fix:** return `Err` on no-match-in-horizon; reject impossible dates at validation.

### B5 — `.expect()` in the health-server thread can abort intent silently **[Verified, Low]**
`supervisor/health/mod.rs` (server thread): `.expect("failed to bind health server TCP listener")` / `.expect("health server crashed")` run on a detached `std::thread`. A bind failure (port in use) panics that thread; the supervisor main loop keeps running with **no control plane and no surfaced error**. 
**Fix:** propagate bind result to the caller (the bind already happens inside the thread *after* `start_health_server` returned `Ok`); bind before spawning and return `Err`.

### B6 — `do_deploy` git-clone failure only logs, continues **[Verified, Low]**
`cli/git/hook.rs`: on `git clone` failure it prints `"Error: git clone failed."` then proceeds to `do_deploy` against a possibly-empty app dir. Should abort the deploy for that ref.

**Runtime-bug sweep:** zombie reaping (`SpawnedProcess::Drop`), process-group kill (`killpg`), and the historical pre_exec/`__ns-shim` deadlock are all correctly handled and well-documented — no issues found there. unwrap/expect in non-test code is otherwise minimal.

---

## Phase 4 — Security Audit

The security model is deliberate and mostly strong: random 256-bit control token (`/dev/urandom`), `0600` perms, constant-time compare, path-traversal canonicalization (`util/validation.rs:ensure_path_within`), nginx value sanitization (`nginx/sanitize.rs`), server-derived (never request-supplied) build paths, namespace + cgroup isolation. Findings:

### S1 — Dashboard is a token-bearing confused deputy with no auth **[Verified, High — deployment-dependent]**
`dashboard/server/routers/control.ts` reads `~/.riku/control.token` server-side and attaches `Authorization: Bearer` to every `/api/control/*` call. The browser-facing Next.js side has **zero authentication** (grep for auth/session/login/csrf finds nothing but the upstream token plumbing), and the API sets **`Access-Control-Allow-Origin: *`** (`app/api/[[...route]]/route.ts`). 
The Rust token correctly stops a browser from hitting the Rust control plane directly — but it does **not** stop a browser from hitting the *dashboard's own* `/api/control/apps/:app` DELETE, which then deputizes the token. Any page the operator visits can issue a cross-site `POST/DELETE` to `http://<dashboard-host>:3000/api/control/...` and **destroy/deploy/stop apps** (CSRF; simple requests don't need to read the response, so CORS doesn't save you). `next start` binds `0.0.0.0` by default, widening exposure to the LAN. 
**Fix:** add real auth on the dashboard (session/login or, at minimum, an operator token + same-origin/CSRF-token check), restrict CORS to the dashboard origin, bind to `127.0.0.1`, and require a custom header on mutating routes.

### S2 — Read-only metrics endpoints are open with `CorsLayer::Any` **[Verified, Low]**
`health/mod.rs`: `/health` + `/metrics*` have `allow_origin(Any)` and no auth on `127.0.0.1`. Documented as intentional (low-sensitivity status). Acceptable, but `/metrics/apps/:app` leaks app names/topology to any local-origin page via DNS rebinding. Low.

### S3 — Cron and release commands run via `sh -c` **[Verified, Low — by design]**
`cron/mod.rs:execute` and `deploy/env_setup.rs:27,93` run `sh -c <command>` from the app's own Procfile/release config. This is arbitrary code execution **by the app owner over their own app** — the intended trust model (like Heroku release phase). Not a vuln, but note: the cron `command` and release commands are never isolated/limited the way long-running workers are (B-class hardening opportunity; cron jobs do get resource limits per `cron_tasks.rs`, release commands do not).

### S4 — Symlink trust in git hook **[Suspected, Low]**
`cli/git/hook.rs`: if `repo_path` is supplied it `symlink`s an arbitrary `actual_repo` into `~/.riku/repos/{app}.git`. The caller is the post-receive hook (server-controlled), so reachability by an attacker is unclear — flag for confirmation that `repo_path` is never attacker-influenced.

**Crypto/secrets:** token generation and comparison are correct. No hardcoded secrets found. CI runs `cargo audit` + `cargo deny` (`deny.toml` denies vulns, restricts to crates.io + license allowlist) — good supply-chain posture.

---

## Phase 5 — Architecture Audit

- **Duplicate cron grammar** (`util/procfile.rs` regex vs `cron/parser.rs`) — two sources of truth for the same domain rule. See B2. Consolidate.
- **Two-tier error strategy half-migrated** — `error.rs` (`thiserror` `DeployError`) used in only ~4 sites; everything else is `anyhow`. The module carries a blanket `#![allow(dead_code)]` masking unused variants. Either finish the migration or trim to the variants actually matched on (`DeployInProgress`, `resource_exhausted`).
- **Custom primitives where mature crates exist** — hex encoding (`format!("{:02x}")`), constant-time compare, `/dev/urandom` read are all hand-rolled in `health/auth.rs` while `rand 0.8.5` is already in the lock file. Defensible (zero-extra-dep, correct) but see Phase 6.
- **47 `#[allow(dead_code)]` markers** across the tree (Phase 7). These suppress the very signal CLAUDE.md's "No Shortcuts" rule wants surfaced. Several whole modules silenced (`error.rs`, `cron/mod.rs`, `config/mod.rs`).
- **Single-threaded supervisor loop** is a deliberate, well-reasoned design (see the excellent `isolation.rs` header comment). Not a flaw, but it means any synchronous work added to the loop blocks all apps — keep that discipline.
- **Layering** is clean overall; no circular deps observed; service layer avoids `println!` in favor of `tracing`.

---

## Phase 6 — Public-Crate Replacement Analysis

| Current Component | Purpose | Recommended Crate | Adoption | Maintenance | Migration |
|---|---|---|---|---|---|
| `cron/parser.rs` (hand-rolled minute-scan + buggy DoW) | cron next-run | **`cron`** (or `croner`/`saffron`) | High | Active | **Medium** — fixes B1/B3/B4 for free; keeps your `CronScheduler` shell |
| `health/auth.rs` `constant_time_eq` | timing-safe compare | **`subtle`** (`ConstantTimeEq`) | Very high | Active | Trivial |
| `health/auth.rs` hex loop | hex encode token | **`hex`** | Very high | Active | Trivial |
| `health/auth.rs` `/dev/urandom` read | CSPRNG bytes | **`rand`** (already a dep) / `getrandom` | Very high | Active | Trivial |
| `util/procfile.rs` cron regex | Procfile cron validation | reuse internal `cron::validate_cron_expression`, or `cron` crate | — | — | Low — deletes duplication |
| Manual ENV file parse (`util/env.rs`) | KEY=VAL with `$VAR` expansion | keep (shell-expansion semantics are app-specific) — **not** `dotenvy` | — | — | N/A (justified custom) |

Everything else (axum, tokio, tera, nix, clap, serde, reqwest, chrono, anyhow, thiserror, once_cell, regex) is already an ecosystem-standard choice. No abandoned/niche crates in use. The custom code that is **justified**: ENV expansion, the namespace `__ns-shim`, log rotation (tailored to riku's layout). The custom code worth replacing: **cron (correctness)**, and the three auth micro-utils (hygiene only).

---

## Phase 7 — Testing Audit

- **~500+ test fns.** Heaviest: `e2e_tests.rs` (41), `smoke_tests.rs` (37), `plugin_tests.rs` (23), plus dense unit tests in `validation`, `env`, `cron`, `executor`, `nginx`, `workers`, `supervisor_ctl`.
- **Strong:** input validation, nginx sanitization, path traversal, supervisor PID handling, hot-reload regression (`daemon/mod.rs` test), plugin discovery, container integration suite (real Ubuntu container, 14,530/14,530 requests, zero zombies — PROJECT_STATUS.md).
- **Gaps / risk-ranked:**
  1. **Cron correctness has tests but they miss B1** — `cron/tests.rs` covers `every_minute`/`hourly` (where `*` day masks the bug) but **no `0 9 * * 1` weekday-only assertion** that would have caught it. **High.**
  2. **No test for the dashboard CSRF/auth model** (S1). **High.**
  3. **Health-server bind-failure path** (B5) untested. **Medium.**
  4. **No performance/regression perf gate in CI** (the container load test is manual, not in `ci.yml`). **Medium.**
  5. Dashboard has Playwright e2e (`tests/e2e-dashboard/`) but coverage of control mutations under auth is unclear. **Medium.**

---

## Phase 8 — Production Readiness

- **Reliability:** strong — zombie reaping, process-group kills, graceful nginx reload, hardened log capture, deploy locking (`deploy/lock.rs`).
- **Observability:** `tracing` throughout; `/health` + `/metrics` + SSE stream; `dump_state` command. Good. Missing: structured/exportable metrics format (Prometheus) — `/metrics` is custom JSON, not the Prometheus exposition format the path name implies.
- **Scalability:** single-node by design (micro-PaaS). Fine for the niche.
- **Fault tolerance:** supervisor survives worker crashes; cron isolated to a bounded thread pool. The detached health-server thread (B5) is the weak link.
- **Deployment/DR:** systemd units, install scripts, container demo env present. No documented backup/restore of `~/.riku` state.

**Production Readiness Score: 72/100** — gated mainly by S1 (dashboard auth) and B1 (cron). Without the dashboard, the core supervisor/deploy engine is ~85.

---

## Phase 9 — Final Report

### Executive Summary

**Top risks**
1. Dashboard confused-deputy / no auth + CORS `*` (S1).
2. Cron fires on wrong days when DoW is set (B1).
3. Procfile rejects valid cron grammar (B2).
4. Detached health-server thread panics silently on bind failure (B5).
5. 47 `#[allow(dead_code)]` masking unused surface (maintenance debt + hides regressions).
6. Cron UTC vs local ambiguity (B3).
7. `/metrics` open + CORS Any leaks topology (S2).
8. Half-finished `error.rs` migration (consistency).
9. No CI perf/load gate (perf regressions can land).
10. Release-phase `sh -c` commands run without resource limits (S3).

**Top bugs:** B1, B2, B5, B3, B4, B6 (above) + cron tests blind to B1 + dashboard untested auth + git-clone-failure continues + impossible-date hourly loop.

**Top missing features:** dashboard authn/authz; Prometheus-format metrics; cron local-TZ + ranges-in-Procfile; backup/restore of state; release-cmd isolation; CI load gate.

**Top architectural concerns:** duplicate cron grammar; error-strategy split; dead-code suppression; custom auth micro-utils; single-loop blocking discipline (manage, don't fix).

### Scores
- **Technical Debt: 28/100** (lower=better) — small, focused files; few real TODOs; debt is dead-code markers + duplication.
- **Security: 74/100** — strong primitives; dashboard auth is the hole.
- **Maintainability: 82/100** — clean layering, strong docs, good tests, small modules per CLAUDE.md rules.
- **Production Readiness: 72/100.**
- **Project Completion: 85/100.**

### Recommended Next 30 Tasks (ranked: impact × risk-reduction ÷ effort)
1. Fix cron DoW/DoM OR-vs-AND logic (B1) + add `0 9 * * 1` test. _High/Low._
2. Add a weekday-only and a list/range cron test matrix.
3. Add dashboard authentication (operator login/token) (S1). _High/Med._
4. Restrict dashboard CORS to its own origin; bind `127.0.0.1`; add CSRF token on mutating routes (S1).
5. Replace cron engine with the `cron` crate (fixes B1/B3/B4). _High/Med._
6. Unify Procfile cron validation onto the engine; delete `CRON_REGEXP` (B2).
7. Bind health listener before spawning the thread; return `Err` on bind failure (B5).
8. Add a regression test for health-server bind failure.
9. Make cron timezone explicit (local or documented UTC) (B3).
10. Reject impossible cron dates at validation; error instead of hourly fallback (B4).
11. Abort deploy on `git clone` failure in the hook (B6).
12. Swap `constant_time_eq` → `subtle`; hex loop → `hex`; token bytes → `rand`/`getrandom`.
13. Audit and remove the 47 `#[allow(dead_code)]`; delete or wire each item.
14. Finish or trim the `error.rs` migration to used variants.
15. Add resource limits to release-phase `sh -c` commands (S3).
16. Expose `/metrics` in Prometheus exposition format (or rename to `/stats`).
17. Add a CI job running the container load suite (perf gate).
18. Document `~/.riku` backup/restore (DR).
19. Add `metrics/apps/:app` access note / consider gating behind token.
20. Confirm `repo_path` symlink in git hook is never attacker-influenced (S4); else validate.
21. Add Playwright tests covering authenticated control mutations.
22. Add an integration test asserting CORS/headers on `/api/control/*`.
23. Rate-limit / log mutating control-plane requests.
24. Add `cargo llvm-cov` coverage reporting to CI.
25. Document the cron grammar actually supported (ranges/lists/steps) in `docs/`.
26. Add graceful-degradation log when control token file is unreadable at startup.
27. Add a smoke test that every bundled plugin's `detect` exits 0 on its fixture.
28. Pin/refresh dashboard deps; add `npm audit` to CI.
29. Add structured request IDs to control-plane tracing for auditability.
30. Add a `--bind` flag so health server can optionally serve beyond loopback behind explicit opt-in (with auth required).

---

## Verification Pass (self-challenge)

- **B1 cron bug — Verified (upgraded).** Traced: `*` → `parse_cron_field` returns full range → `contains` always true → `||` nullifies weekday. The existing passing cron tests use `*` day, so they cannot expose it. High confidence.
- **S1 dashboard — Verified, but severity is deployment-dependent.** If the dashboard is never run, or strictly bound to loopback for a single trusted local user, real-world impact drops to Medium. Kept High because `next start` defaults to `0.0.0.0` and there is no auth code at all. Not over-claiming RCE — it's CSRF-grade abuse of existing control actions.
- **"Dead code" — softened.** `cron/mod.rs` and `error.rs` are **used** (cron wired at `daemon/mod.rs:257`; `DeployError` matched at `health/control.rs:90`). The `#![allow(dead_code)]` covers *some* unused members, not whole-module death. Reworded from "dead module" to "masked unused surface." 
- **unwrap/panic — downgraded.** Initial grep flagged many; nearly all are in `#[cfg(test)]` blocks. Only the health-thread `.expect()` (B5) is a genuine production concern. Removed as a top finding, kept as B5.
- **Custom crypto — downgraded to hygiene.** `constant_time_eq` and the `/dev/urandom` token are **correct**; recommending `subtle`/`rand` is cleanliness, not a vulnerability. Not security-critical.
- **Removed weak/unverifiable claims:** no assertion about DST (UTC sidesteps it), no SQL/XSS/SSRF claims (no DB; dashboard renders trusted local data; no user-controlled outbound fetch found). S4 left as **Suspected** pending confirmation of `repo_path` provenance.

**Net:** Two findings genuinely move the needle — **B1 (cron correctness)** and **S1 (dashboard auth)**. Everything else is polish on an already-disciplined codebase.

# Riku — Independent Repository Audit (verification pass)

**Date:** 2026-06-23 · **Commit audited:** `7675359` (HEAD, `feat/dashboard-control-plane`)
**Auditor role:** Staff Engineer / Security / Architecture / QA / OSS maintainer
**Scope:** full Rust workspace (`src/`, `crates/`), Next.js dashboard, CI, tests, deps.

---

## 0. Headline

This audit was run against a tree that **already contains a prior in-repo audit** (`AUDIT_REPORT.md`, commit `e39de21`). That report is **stale**: the remediation commits landed in the *same push*, timestamped `16:32:29`, **16 seconds before** the report doc itself (`16:32:45`). Every High/Medium finding it lists as "Verified" has already been fixed at HEAD. Treat `AUDIT_REPORT.md` as a historical record of a *previous* state, not the current one.

**Verdict:** mature, security-conscious systems codebase. Not a prototype. Baseline quality high. Remaining open items are **Low** severity. No Critical/High issues found at HEAD.

| Score | Value |
|---|---|
| Project Completion | **88 / 100** |
| Security | **86 / 100** |
| Maintainability | **85 / 100** |
| Production Readiness | **80 / 100** |
| Technical Debt (0=clean, 100=drowning) | **22 / 100** |

---

## Phase 1 — Discovery

**Stack:** Rust 2021 single binary. clap 4 (CLI) · axum 0.7.9 + tokio 1.49 (control/health HTTP) · tera 1.20 (nginx templates) · nix 0.29 + libc (namespaces/signals/cgroups/rlimits) · serde/serde_json · anyhow + thiserror · chrono · croner 2 (cron) · regex · reqwest 0.12 (blocking, rustls — `default-features=false`) · subtle/hex/rand (control token) · shell-words · threadpool · notify 8 (config watch). Dashboard: Next.js 16 / React 19 / Hono 4 / Tailwind v4 / Playwright, separate Node service. Runtime plugins: shell scripts (`plugins/node|python|ruby|go|rust-lang`) + 3 Rust binary crates (java, clojure, container).

**Workspace:** root + 3 plugin crates, `resolver = "2"`. Release profile: `lto=true`, `strip=true`, `codegen-units=1`. ~26.8k LOC Rust (incl. tests).

**Layering** (CLAUDE.md contract) is largely honored: `cli/` + `plugins/` (provider) → `deploy/` + `supervisor/` + `nginx/` (service) → `config/` (repository).

**Critical paths:**
- Deploy: SSH `git push` → `cli/git/receive_pack.rs` → post-receive `cli/git/hook.rs` → `deploy::do_deploy` → runtime plugin `detect/build/env/start` (`plugins/runtime.rs`, `executor.rs`) → worker TOML → supervisor.
- Supervisor: `supervisor/daemon/mod.rs:run` single-threaded event loop (SIGHUP reload, health, log rotation, cron, stats).
- Control plane: axum on `127.0.0.1:<port>` — open read-only `/health` `/metrics*`, token-gated `/control/*`.
- Dashboard: browser → Next.js `/api/*` (Hono) → Rust control plane (server-attached bearer token).

**CI** (`.github/workflows/ci.yml`): fmt → `clippy -D warnings` → build → test → release build → shell deploy tests → `cargo audit` → `cargo deny check`. `deny.toml` restricts licenses + registries. Strong gate. Hardening lints denied workspace-wide (zombie-process / `mem_forget`) — compile-time enforcement of the resource-leak class this codebase is exposed to.

---

## Phase 2 — Completion

| Feature | Status | Confidence | Evidence |
|---|---|---|---|
| git-push deploy (SSH) | Complete | Verified | `cli/git/*`, `deploy/`, container integration test pass |
| Runtime plugin protocol (detect/build/env/start) | Complete | Verified | `plugins/runtime.rs`, `executor.rs`, bundled `plugins/*` |
| Process supervisor (spawn/health/restart/stop) | Complete | Verified | `supervisor/process/*`, `daemon/mod.rs` |
| Namespace (mnt/net/pid) + cgroup v2 isolation | Complete | Verified | `process/isolation.rs`, `cgroups/`, `__ns-shim` |
| Resource limits (rlimit) | Complete | Verified | `supervisor/resource_limits/` |
| Log rotation | Complete | Verified | `supervisor/log_rotation/` |
| nginx config gen + SSL/ACME/Cloudflare | Complete | Verified | `nginx/`, `templates/*.tera` |
| Cron scheduler | **Complete** | Verified | now `croner`-backed, Vixie DoM/DoW semantics, single validator shared with Procfile (`supervisor/cron/`) — *prior bug fixed in `881ffc4`* |
| Control-plane HTTP API + token auth | Complete | Verified | `health/control.rs`, `health/auth.rs` |
| Dashboard (Next.js) | **Complete (auth added)** | Verified | operator token + CSRF/same-origin guard on mutating routes (`server/security.ts`) — *prior "no auth" fixed in `37a4825`* |
| Container build/export + remote deploy | Complete | Verified | `deploy/container_runtime.rs`, `crates/riku-plugin-container` |
| Typed error domain (`error.rs`) | Adequate | Verified | `DeployError` 2 variants, both used; unused variants removed in `b0e2e82`. Rest of codebase intentionally `anyhow` |

**Completion:** Overall **~88%**. Backend/CLI ~92%. Supervisor ~90%. Dashboard ~85%. Infra/CI ~90%. Testing ~82%. Docs ~85%.

Stubs/placeholders sweep: only `stats/resources.rs` has explicit non-Linux stubs (documented, intentional). `executor.rs:57 unreachable!()` is genuinely unreachable (loop returns on its final iteration) — not a defect. No `todo!`/`unimplemented!` in shipping code.

---

## Phase 3 — Bugs (at HEAD)

The prior report's B1–B6 are **resolved at HEAD**:

- **B1 cron DoM/DoW** — fixed: croner provides correct Vixie OR/AND semantics (`881ffc4`).
- **B2 Procfile-vs-scheduler grammar mismatch** — fixed: both route through `validate_cron_expression` (single source of truth, `util/procfile.rs` + `supervisor/cron/`).
- **B5 health-server `.expect()` on detached thread** — fixed: listener bound synchronously before spawn, bind failure returned as `Err` (`health/mod.rs`, `1c104fb`).
- **B6 `do_deploy` continuing after clone failure** — fixed: `cli/git/hook.rs:57` now returns `Err` on clone failure (`7b4911c`).

**Remaining (Low):**

| ID | Sev | Location | Issue |
|---|---|---|---|
| BUG-1 | Low | `supervisor/cron/` | Schedules are **UTC-only** by design; no per-app TZ. Documented, but a foot-gun for operators expecting local wall-clock. Feature gap, not a logic bug. |
| BUG-2 | Low | dashboard same-origin guard | CSRF defense leans on `Origin`/`Sec-Fetch-Site` headers + token. Correct for modern browsers; non-browser/legacy clients bypassing those headers still need the token, so net risk is low — but worth a regression test. |

Runtime-bug sweep (zombies, `killpg` process-group kill, the historical `pre_exec`/`__ns-shim` deadlock, `ETXTBSY` exec race retry) — all correctly handled and documented. No panics/leaks/deadlocks found in shipping paths. `unwrap`/`expect` in non-test code is minimal and confined to genuinely-infallible spots.

---

## Phase 4 — Security (at HEAD)

Model is deliberate and strong: 256-bit control token from `OsRng` → `0600` file, `subtle` constant-time compare; path-traversal canonicalization (`util/validation.rs:ensure_path_within`); nginx value sanitization (`nginx/sanitize.rs`); server-derived (never request-supplied) build paths; namespace + cgroup isolation; `reqwest` with rustls, no OpenSSL.

Prior **S1 (dashboard confused-deputy / CSRF)** and **S2 (CORS `Any`)** are **fixed at HEAD**:

- `dashboard/server/security.ts`: `requireMutatingAuth` enforces (1) operator token in `x-riku-dashboard-token`, constant-time (`timingSafeEqual`), **fail-closed 503 if `RIKU_DASHBOARD_TOKEN` unset**; (2) `Origin` must equal `RIKU_DASHBOARD_ORIGIN`, Origin-less requests accepted only with `Sec-Fetch-Site: same-origin`. Mounted on the whole `/control/*` group + mutating env routes.
- CORS locked to the single configured origin on **both** sides — dashboard (`app/api/.../route.ts`) and Rust (`health/mod.rs:readonly_cors_layer`, no more wildcard).
- `/csrf` token-delivery endpoint is itself same-origin-gated; token never enters the JS bundle / `NEXT_PUBLIC_*`.

| ID | Sev | Status | Note |
|---|---|---|---|
| S1 dashboard CSRF/no-auth | High→**Resolved** | Verified | `37a4825`, `security.ts` |
| S2 metrics CORS `Any` | Low→**Resolved** | Verified | `1c104fb` |
| S3 cron/release via `sh -c` | Low | By design | Commands are operator-authored (Procfile/release), not attacker-supplied; same trust level as the deploy itself |
| SEC-1 `RUSTSEC-2026-0097` (`rand 0.8.5` unsound) | **Informational** | Verified | Advisory targets `rand::rng()` global RNG paired with a custom logger. Riku uses **`OsRng` directly** (`health/auth.rs:48`) and pulls `rand` transitively via `tera`→`chrono-tz-build`. The vulnerable code path is never exercised. Bump when `tera` updates; no action urgent. |

**Residual security posture is deployment-dependent:** the dashboard is only safe if the operator (a) sets `RIKU_DASHBOARD_TOKEN`, (b) binds `next start -H 127.0.0.1`, (c) sets matching `RIKU_DASHBOARD_ORIGIN` both sides. These are documented in `route.ts` but enforced only by fail-closed defaults, not by the harness. Recommend a startup preflight that refuses to serve if bound to `0.0.0.0` without a token.

---

## Phase 5 — Architecture

Clean. Service/Provider/Repository layering respected; files kept small per the ≤200-LOC rule (largest non-test source `process/spawn.rs` 639, `deploy/workers.rs` 546 — candidates to split but not violations of intent). No circular deps. Cron now has a single validator shared by Procfile and scheduler (good de-duplication). `anyhow` for leaf failures + typed `DeployError` where callers must branch (e.g. 409 vs 500) is a reasonable, deliberate split — not "partial adoption" debt.

Minor:
- ARCH-1 (Low): `process/spawn.rs` and `deploy/workers.rs` exceed the self-imposed 200-LOC guideline — split for readability.
- ARCH-2 (Low): two `// SECURITY`-relevant invariants (loopback bind, token presence) live in comments, not code preconditions. Promote to runtime asserts.

---

## Phase 6 — Public-crate replacement

The codebase has **already** internalized the standard ecosystem and recently *removed* its homegrown reinventions (custom cron engine → `croner`; hand-rolled token compare → `subtle`/`hex`/`rand`). Little remains to replace.

| Current component | Purpose | Recommended crate | Adoption | Maintained | Migration |
|---|---|---|---|---|---|
| `stats/resources.rs` `/proc` parser | Per-process CPU/RSS via `/proc` | **`procfs`** | High | Active | Medium — would remove platform-stub code, but adds a dep for something narrow & working. Optional. |
| `util/procfile.rs` env-var regex expansion | `$VAR` / `${VAR}` substitution | `shellexpand` | Medium | Active | Low — but current regex is tiny and correct; not worth it. |
| `executor.rs:spawn_retrying_etxtbsy` | bounded exec retry | (keep hand-rolled) | — | — | Don't replace: `backoff`/`tokio-retry` are async/heavier; this 15-line sync helper is the right size. |

**Conclusion:** no compelling replacements. The only marginally-justified one is `procfs`. Everything else is mature crates already (clap, axum, tower-http, tokio, serde, tracing, croner, subtle, reqwest, chrono, regex, notify, threadpool). This phase yields no action item of consequence — a sign the dependency choices are already good.

---

## Phase 7 — Testing

Broad: 11 integration test files (e2e 1412 LOC, resilience 753, plugin 641, nginx 577, supervisor 512, security 478, smoke 450, regression 432) + stress tests + dense unit `#[cfg(test)]` modules across `deploy/`, `supervisor/`, `nginx/`, `util/`, `plugins/`. Cron has dedicated tests incl. impossible-schedule rejection (`0 0 30 2 *`) and out-of-range fields.

Gaps (risk-ranked):
- **T-1 (Med):** dashboard `security.ts` auth/CSRF has no automated test — the newest, highest-value security boundary is unverified by CI. Add Playwright/unit coverage for: missing token → 503, wrong token → 401, cross-origin → 403, `Sec-Fetch-Site` logic.
- **T-2 (Low):** no performance/load test for the SSE metrics broadcast under many clients.
- **T-3 (Low):** UTC-only cron behavior not asserted against a non-UTC `TZ` env.

---

## Phase 8 — Production Readiness (80/100)

Strong: structured `tracing` (env-filter + JSON), `/metrics` + SSE stream, health endpoint, cgroup/rlimit fault isolation, graceful shutdown, zombie reaping, log rotation hardened against external rotators, generation-based deploy orchestration, deploy lock hardened vs EINTR (`7675359`).

Gaps: no alerting layer (metrics are exposed, not wired to anything — expected for this tier); dashboard safety is operator-config-dependent (see Phase 4); no documented DR/backup story for `~/.riku/` state; single-node by design (not a fault).

---

## Phase 9 — Final

### Top risks (all Low/Informational at HEAD)
1. Dashboard safety depends on operator setting token + loopback bind (defaults fail-closed, but no preflight).
2. `security.ts` CSRF boundary untested in CI (T-1).
3. `rand 0.8.5` advisory present in tree (unreached path).
4. UTC-only cron surprises operators.
5. Large source files drifting past the 200-LOC guideline.
6. No DR/backup guidance for state dir.
7. SSE broadcast unbenchmarked under load.
8. Security invariants encoded in comments, not asserts.
9. In-repo `AUDIT_REPORT.md` is stale/misleading — documents already-fixed issues as open.
10. `procfs` parsing is hand-rolled (works; minor).

### Top bugs
B1–B6 from the prior report are **fixed**; only BUG-1 (UTC-only) and BUG-2 (header-dependent CSRF) remain, both Low. No Critical/High bugs.

### Top missing features
Per-app cron timezone; dashboard automated auth tests; startup preflight refusing unsafe bind; DR/backup tooling; alerting hooks; multi-node (out of scope by design).

### Recommended next tasks (impact × risk-reduction × effort)
1. **Add CI tests for `security.ts`** (token 503/401, origin 403, Sec-Fetch-Site) — high value, low effort. *(T-1)*
2. **Startup preflight**: refuse to serve dashboard if bound non-loopback without `RIKU_DASHBOARD_TOKEN`. *(S1 residual)*
3. **Delete or date-stamp `AUDIT_REPORT.md`** so it stops contradicting the code. *(finding #9)*
4. Bump `tera` when a release drops `rand 0.8`, clearing `RUSTSEC-2026-0097`. *(SEC-1)*
5. Document/optionally support per-app cron TZ. *(BUG-1)*
6. Split `process/spawn.rs` / `deploy/workers.rs` below 200 LOC. *(ARCH-1)*
7. Promote loopback-bind + token invariants to runtime asserts. *(ARCH-2)*
8. Add SSE broadcast load test. *(T-2)*
9. Document `~/.riku/` backup/restore (DR). *(readiness)*
10. Evaluate `procfs` to drop platform-stub code. *(optional)*

---

## Verification pass (self-challenge)

| Claim | Mark | Evidence |
|---|---|---|
| Dashboard auth/CSRF present | **Verified** | read `server/security.ts` in full |
| CORS locked both sides | **Verified** | `route.ts` + `health/mod.rs:readonly_cors_layer` |
| Cron uses croner, correct semantics | **Verified** | `supervisor/cron/` source + tests |
| Health bind error propagated | **Verified** | `health/mod.rs` synchronous bind |
| Clone failure aborts deploy | **Verified** | `cli/git/hook.rs:57` returns `Err` |
| `rand` advisory unreached | **Verified** | only `OsRng` used (`auth.rs:48`); advisory targets `rand::rng()` |
| `node_modules` not committed | **Verified** | `git ls-files dashboard/node_modules` = 0; gitignored |
| `executor.rs unreachable!` is sound | **Verified** | loop returns on final iteration |
| Prior `AUDIT_REPORT.md` is stale | **Verified** | fix commits `16:32:29` precede report `16:32:45` |
| UTC-only cron is a foot-gun | **Likely** | documented intentional; impact is operator-dependent |
| `procfs` worth adopting | **Suspected** | current parser works; benefit is marginal |

**Downgraded from prior report:** S1 High→Resolved, S2 Low→Resolved, B1 High→Resolved, B2/B5/B6→Resolved. **No findings upgraded.** No guesses retained.

---

## Addendum — Dashboard deep-dive + hardening (same day)

A focused second pass over `dashboard/` (server routers, app API routes, lib, components, tests) surfaced filesystem-touching read routes that the repo-level pass had not drilled into. All were **fixed and verified** (tsc clean, 27 unit tests passing) the same day.

| ID | Sev | Issue | Fix |
|---|---|---|---|
| D1 | High* | `GET /api/env/:app` returned env **secrets** gated by CORS only — no token | Gated whole `/env/*` group with `requireMutatingAuth` (`server/routers/env.ts`) |
| D2 | Med | `logs/stream?app=` raw query param → `path.join` traversal, unauth | Validate via shared `validateAppName` before any path/`watch` op |
| D3 | Med | env `PUT/DELETE` app name unvalidated → traversal write (auth-gated) | Validate before `mkdir`/`writeFile` |
| D4 | Med | No app-name validation parity Rust↔Node (root cause) | New `dashboard/server/validation.ts` mirroring Rust charset `[A-Za-z0-9._-]`, strict-reject `..`/`/`/empty/dot-only |
| D5 | Low/Med | No `next.config` → no CSP / anti-clickjacking headers | New `next.config.ts`: CSP, `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy` |
| D6 | Low | env error responses leaked fs paths via `detail` | Dropped `detail`; log server-side |
| D7 | Low | dual lockfiles (`package-lock.json` + `pnpm-lock.yaml`) | Removed npm lockfile; pnpm is canonical |
| D8 | Med | zero tests on the auth/validation boundary | `tests/unit/{validation,security}.spec.ts` — 27 cases; `unit` Playwright project + `test:unit` script |

*D1 severity is deployment-dependent (exposure scales with non-loopback bind).

**Also added:** `dashboard/instrumentation.ts` startup preflight — refuses to boot when `RIKU_DASHBOARD_TOKEN` is unset and the server is either in production or bound to a non-loopback host; warns on token-less loopback dev. Closes the "operator forgets the token while exposed" gap (recommended task #2).

**Clean (verified absent):** no XSS sinks (`dangerouslySetInnerHTML`/`innerHTML`/`eval`), no `child_process`/`exec`, all upstream fetches time-bounded, control token never reaches the browser.

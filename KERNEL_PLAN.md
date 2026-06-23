# Riku Kernel & Plugin Refactor — Brief Plan

> Status: **draft for alignment**. Decisions marked _(proposed)_ are defaults
> pending sign-off, not settled. This plan is deliberately short; detail lands
> per-phase as we build.

## 1. Goal

Turn Riku into a **small kernel + a known set of versioned seams**. Everything
user-facing becomes a plugin over the kernel. We **retrofit**, not rewrite — the
current code already contains most of the kernel. The vision is "feels pluggable
to anything," delivered as a disciplined contract, not an open RCE surface.

Guiding line (from `ROADMAP.md`): _freeze a small, stable API rather than
chasing a rich, unstable one._

## 2. The Kernel (never a plugin)

- **Git intake** — accepts the push, triggers deploy.
- **Process supervisor** — owns PIDs, health, restarts.
- **State contract** — `RikuPaths`, ENV files, worker TOML on disk.
- **Plugin registry / loader** — discovery, manifest parsing, dispatch.
- **Lifecycle event bus** — typed events; the generalization of today's 4 hooks.

Everything else is a plugin: runtime, addon, router, notifier, auth, hook.

## 3. Seam Model _(resolved → see `PLUGIN_PROTOCOL.md`)_

Decided. The full contract lives in **`PLUGIN_PROTOCOL.md`** (`RIKU_PLUGIN_API=1`).
Summary:

- **Three behavior seams**: **runtime** (exists), **addon** (keystone), **router**.
  These are where plugins contribute behavior, via a typed verb-set per seam.
- **One event bus** subsumes the old `hook` and `notifier` types — they become
  marketplace *categories* that resolve to event subscribers, not distinct
  mechanisms.
- **Power is bounded by trust**: `observe`-mode subscribers are open to any
  plugin; `gate`-mode (veto) is allowed only on pre-phase events and requires an
  elevated capability + trusted marketplace.
- **Auth seam deferred to v2** — only matters once the dashboard ships; keeping
  v1 small is the discipline that keeps the contract stable.

## 4. Event Bus _(proposed contract)_

- Kernel emits typed events as **JSON lines** with a versioned schema
  (`RIKU_PLUGIN_API`). Examples: `deploy.started`, `build.finished`,
  `release.activated`, `app.scaled`, `app.stopped`.
- Subscribers declared in the plugin manifest (which events, observe-vs-veto).
- The existing 4 hooks (`pre/post` × `deploy/build`) become 4 events among many
  — emitted at the **same points**, so step 1 is zero behavior change.
- Open question: sync veto vs async fire-and-forget per event class (default:
  lifecycle gates are sync/vetoable, notifications are async).

## 5. Dev Orchestration (mise) — _build before the refactor_

Adopt [`mise`](https://mise.jdx.dev) to pin tools and codify every workflow as a
task, so local == CI. Mirrors today's CI exactly, just one entry point.

- **Pinned tools:** rust (toolchain), `cargo-nextest` (test runner),
  `cargo-llvm-cov` (coverage, replaces ad-hoc `coverage.sh`), `cargo-audit`,
  `cargo-deny`.
- **Tasks (`mise.toml`):**
  - `fmt` / `fmt:check` — `cargo fmt`
  - `lint` — `cargo clippy -- -D warnings`
  - `build` / `build:release`
  - `test` — unit + integration (nextest)
  - `test:deploy` — `tests/run-all-tests.sh`
  - `cov` — coverage report
  - `audit` / `deny` — supply-chain checks
  - `ci` — aggregate (`fmt:check` + `lint` + `build` + `test` + `audit` + `deny`)
- CI workflow shrinks to "install mise, run `mise run ci`."

## 6. Refactor Phases (build order)

1. **✅ Contract v1 + event schema** — `RIKU_PLUGIN_API=1`, event enum → JSON
   schema, emitted at the existing hook points. Zero behavior change.
2. **✅ Event bus + notifier seam** — `riku-plugin.toml` manifest parsing and
   subscribe-to-events dispatch (`on_event` + JSON on stdin), observe-mode.
   Example `webhook-notify` notifier under `examples/plugins/`. (gate-mode veto
   deferred to the trust slice; a gate subscriber runs as observe and says so.)
3. **Addon seam (Postgres)** — the keystone: provision / bind / unbind, exercises
   lifecycle events + capability declaration + env injection + state.
4. **Distribution + trust** — marketplace, lockfile, checksum/capability
   enforcement (per `ROADMAP.md` E2 / E2.5).

## 7. Decisions

- [x] **Seam model** — three behavior seams + event bus; `observe` open /
      `gate` elevated. Spec'd in `PLUGIN_PROTOCOL.md`.
- [x] **Wire format** — argv-verb + env-context + JSON-over-stdio for structured
      seams; line-oriented stays valid for simple verbs (`PLUGIN_PROTOCOL.md` §4).
- [x] **Event veto semantics** — only pre-phase events gateable; gate timeout =
      fail-closed veto (`PLUGIN_PROTOCOL.md` §7.2).
- [x] **mise tool set** — nextest + llvm-cov + audit + deny adopted (§5).
- [x] **First slice** — Contract v1 + event schema. `RIKU_PLUGIN_API=1`,
      `EventName`/`EventEnvelope` (JSON-line wire form), emitted at the four
      legacy hook points with zero behavior change. Subscriber dispatch +
      manifest parsing + the rest of the event catalog land in slice 2.

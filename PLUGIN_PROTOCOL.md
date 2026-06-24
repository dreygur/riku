# Riku Plugin Protocol v1

> **Status:** design spec for `RIKU_PLUGIN_API = 1`. This is the stable contract
> the kernel/seam refactor (see `KERNEL_PLAN.md`) is built against. Sections
> marked _(exists)_ describe current behavior being formalized; _(new)_ marks
> additions. The contract is intentionally small — see §9 on what is *not* a
> seam.

## 1. Model

Riku is a **kernel** plus a small set of **seams**. The kernel owns the parts
that can never be a plugin; plugins extend Riku in exactly two ways:

- **Behavior seams** — the kernel calls *into* the plugin to get work done.
  v1 seams: **runtime**, **addon**, **router**.
- **Event subscribers** — the plugin reacts to lifecycle **events** the kernel
  emits. This subsumes the old `hook` and `notifier` types: they are no longer
  distinct mechanisms, only marketplace *categories* that resolve to event
  subscribers.

```
            ┌──────────────────────── kernel ────────────────────────┐
 git push → │ git intake → deploy orchestration → supervisor          │
            │ state (RikuPaths/ENV/worker TOML) · registry · event bus │
            └───┬──────────────┬──────────────┬───────────────┬───────┘
        seam:runtime    seam:addon      seam:router      event bus
        (detect/build/  (provision/     (configure/      (observe / gate)
         env/start)      bind/…)          reload)
```

## 2. Versioning

- `RIKU_PLUGIN_API` is a single integer. This document defines **1**.
- The kernel sets `RIKU_PLUGIN_API=1` in every plugin invocation's environment.
- A plugin's manifest declares `api = 1`. The kernel refuses to load a plugin
  whose declared `api` it does not support.
- **Additive** changes (new seam, new event, new optional manifest field) stay
  within an API version. **Breaking** a verb's I/O or an event's payload bumps
  the version. The kernel may support multiple versions simultaneously.
- Legacy runtime plugins with no manifest are treated as **api = 0** and keep
  working via the argv+env+line protocol (§8).

## 3. Bundle & manifest

A plugin is a directory (git repo or subdir), not a single file:

```
my-plugin/
  riku-plugin.toml      # manifest
  bin/                  # executable(s) implementing the seam's verbs
  README.md
```

```toml
# riku-plugin.toml
name        = "postgres"
version     = "1.2.0"
type        = "addon"            # runtime | addon | router | notifier | hook
api         = 1
entry       = "bin/riku-postgres"
checksum    = "sha256:..."       # verified on install
description = "Managed PostgreSQL addon"
author      = "..."

[capabilities]                   # declared, shown on install, enforced where possible
network     = true
writes      = ["app_dir", "data_dir"]
privileged  = false

[events]                         # present iff the plugin subscribes to events
subscribe   = ["deploy.finished", "deploy.failed"]
mode        = "observe"          # observe | gate  (gate needs elevated trust, §7)
```

`type` is the *category*. `runtime`/`addon`/`router` bind the plugin to a
behavior seam (§5–§6). `notifier`/`hook` are categories whose behavior is
entirely defined by the `[events]` block — they implement no seam verbs.

## 4. Invocation model

Every call is a fresh process:

- **Verb** is `argv[1]` (e.g. `detect`, `provision`, `on_event`).
- **Context** is passed via environment variables. Always present:
  `RIKU_PLUGIN_API`, `RIKU_ROOT`. App-scoped calls add `RIKU_APP`,
  `RIKU_APP_PATH`, `RIKU_ENV_PATH`. Seam-specific vars are listed per seam.
- **Structured input** (when a verb needs it) is a single JSON document on
  **stdin**. Simple verbs may ignore stdin entirely.
- **Output:**
  - **exit code** — `0` = success / affirmative; non-zero = failure / negative.
    `detect` and event `gate` decisions are expressed purely by exit code.
  - **stdout** — either a single line (legacy runtime `start`/`env`) or a JSON
    object, as the seam specifies. Never mix the two for one verb.
  - **stderr** — human-readable logs; streamed to the deploy log live.
- **Timeout** — the kernel enforces a per-verb timeout (reusing the existing
  timeout-aware executor). A timed-out call is a failure.

JSON-over-stdio is the canonical format for verbs that exchange structured data
(addon credentials, event payloads). Verbs that only need a yes/no or a single
line stay line-oriented, so shell authors keep a trivial path.

## 5. Seam: runtime _(exists — formalized)_

Builds and runs an app. Many installed; selected per app by detection.

| Verb     | Input                | Output                          | Notes |
| -------- | -------------------- | ------------------------------- | ----- |
| `detect` | app context (env)    | exit 0 ⇒ "I handle this app"    | Selection: `RUNTIME=<name>` override → else alphabetical, first exit-0 wins. |
| `build`  | app context          | logs on stderr; non-zero aborts | Compile/install deps. |
| `env`    | app context          | `KEY=VALUE` lines on stdout     | Injected into the app environment. |
| `start`  | app context          | one command line on stdout      | The process the supervisor runs. |

Env: `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`. No persistent
state of its own.

## 6. New seams

### 6.1 addon _(new — the keystone)_

A managed resource (database, cache, queue). Many installed; each creates
**named instances** (`riku addon create postgres db1`). Holds secrets and owns a
data directory — the highest-trust seam.

| Verb          | Input (stdin JSON)        | Output (stdout JSON)                    |
| ------------- | ------------------------- | --------------------------------------- |
| `provision`   | `{instance, plan?}`       | `{}` / status — creates the resource    |
| `bind`        | `{instance, app}`         | `{"env": {"DATABASE_URL": "..."}}`      |
| `unbind`      | `{instance, app}`         | `{}` — kernel removes the injected vars |
| `deprovision` | `{instance}`              | `{}` — destroys the resource (guarded)  |
| `backup`      | `{instance}`              | `{"artifact": "<path>"}`                |

On `bind`, the kernel merges the returned `env` into the app's ENV; on `unbind`
it removes exactly those keys. Env: `RIKU_ADDON_INSTANCE`,
`RIKU_ADDON_DATA_PATH` (under `data_root/addons/<plugin>/<instance>/`), plus app
context for bind/unbind.

### 6.2 router _(new)_

Exposes apps to the network. **Singleton** — exactly one router is active,
chosen by config (`RIKU_ROUTER=nginx|caddy|…`, default `nginx`). Today's built-in
nginx generation is the default router; the seam lets it be swapped.

| Verb        | Input (stdin JSON)                                   | Output         |
| ----------- | ---------------------------------------------------- | -------------- |
| `configure` | `{app, domains, upstream_port, https, …}`            | writes config  |
| `reload`    | —                                                    | reloads router |

Owns its config directory. Env: `RIKU_APP`, `RIKU_ROOT`.

## 7. Event bus _(new)_

The kernel emits typed lifecycle events. Subscribers are declared in the
manifest `[events]` block (§3) and invoked with verb `on_event` and the event
JSON on stdin:

```json
{
  "api": 1,
  "event": "deploy.finished",
  "ts": "2026-06-24T12:00:00Z",
  "app": "myapp",
  "data": { "release": "r42", "runtime": "node", "duration_ms": 1234 }
}
```

### 7.1 Catalog (v1)

| Event                | Phase | Gateable | Legacy hook    |
| -------------------- | ----- | -------- | -------------- |
| `deploy.requested`   | pre   | **yes**  | `pre-deploy`   |
| `build.started`      | pre   | **yes**  | `pre-build`    |
| `build.finished`     | post  | no       | `post-build`   |
| `build.failed`       | post  | no       | —              |
| `release.activated`  | post  | no       | —              |
| `deploy.finished`    | post  | no       | `post-deploy`  |
| `deploy.failed`      | post  | no       | —              |
| `app.scaled`         | post  | no       | —              |
| `app.stopped`        | post  | no       | —              |
| `app.restarted`      | post  | no       | —              |

The four legacy hooks are emitted at the **same points** as today — step 1 of
the refactor is to emit these events there with **zero behavior change**.

### 7.2 Modes — power is bounded by trust

- **observe** (default) — fire-and-forget. Runs async; failures are logged, never
  fatal. **Open to any plugin** (a Slack notifier is harmless).
- **gate** — only valid on **pre-phase / gateable** events. A non-zero exit
  **vetoes** the action (e.g. block a deploy on a failing migration check).
  Requires the elevated `events.gate` capability and a trusted marketplace
  (§3, and the trust model in `ROADMAP.md` E2.5). Gated events block until all
  gaters return or time out; a timeout is a veto (fail-closed), surfaced as an
  install-time/config error rather than a silent stall.

This is the core security idea: **observers are open and safe; only behavior
that can change or block an action requires explicit trust.** The security model
falls out of the seam model instead of being bolted on.

## 8. Backward compatibility

Existing runtime shell plugins have no manifest. They are loaded as **api = 0
legacy runtimes** and keep working unchanged via argv+env and line output (§5).
Events, capabilities, addon/router seams, and gating all require a manifest with
`api = 1`. No existing deployment breaks when v1 lands.

## 9. What is deliberately *not* a seam (v1)

A thing earns seam status only when there is real demand for a *second*
implementation. These stay in the kernel for v1:

- **git intake** and **deploy orchestration**
- **process supervisor** (health checks, restarts, scaling)
- **on-disk state** (`RikuPaths`, ENV, worker TOML)
- **dashboard auth** — deferred to v2; only matters once the dashboard ships,
  and keeping v1 small is the discipline that makes the contract stable.

## 10. Security summary

- Checksum verified on install; manifest mismatch is rejected.
- **Author signatures (optional).** The manifest may carry a hex `signature` —
  an Ed25519 signature over the entry executable's bytes. The operator trusts
  publisher keys with `riku plugins trust add <name> <pubkey>`; on install a
  signed bundle is accepted only if some trusted key verifies it, and otherwise
  **rejected** (not merely warned). Authors use `riku plugins keygen` /
  `riku plugins sign`. The verifying key's name is pinned in the lockfile.
- **Capability enforcement.** Capabilities are declared in the manifest, shown
  on install, and enforced on every manifest-based plugin spawn (addon seam,
  event subscribers) where the platform allows:
  - `privileged = false` (default) sets `PR_SET_NO_NEW_PRIVS`, blocking setuid
    privilege escalation.
  - `writes = [app_dir|data_dir|env_dir]` confines the plugin's filesystem
    writes to exactly those directories (plus the system temp dir and `/dev`)
    via **Landlock**; read/execute stays unrestricted.
  - `network = false` denies TCP bind/connect via Landlock network rules
    (kernel ≥6.7). UDP/non-TCP is not yet covered.

  Enforcement is unprivileged and **best-effort**: on a kernel without Landlock
  the filesystem/network limits degrade to a logged no-op while `no_new_privs`
  still applies. Legacy manifest-less runtime plugins (§8) are not sandboxed.
- `gate` mode and `privileged` capability require explicit, elevated trust;
  third-party marketplaces are opt-in. `privileged = true` opts a plugin **out**
  of the sandbox, so it is the one capability that widens authority.
- WASM sandboxing for untrusted authors is the long-term answer (`ROADMAP.md`
  E3), not a v1 requirement.

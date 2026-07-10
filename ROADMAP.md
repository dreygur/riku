# Riku Roadmap

Two goals drive this roadmap:

1. **Solo-dev friendly** — make Riku the nicest single-box, no-Docker, git-push PaaS for an individual developer.
2. **Vast plugin ecosystem** — turn Riku's extension points into a real ecosystem with a stable contract, many plugin types, and one-command discovery/install.

Both goals respect Riku's identity: a single Rust binary, no runtime dependencies, no Docker required, runs on one small box (VPS / SBC / homelab).

This is a living document. Phases are ordered by leverage, not by size.

---

## Track A — Solo-Dev DX (adoption funnel)

### Phase 0 — First-5-Minutes Magic (highest leverage) — **shipped**

All three landed together in `41bb349`:

- ✅ **One-line installer** (`scripts/install.sh`) — `curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh`. Detects OS/arch, downloads and checksum-verifies the matching release binary, installs it, and prints a `riku init` hint (systemd + nginx setup still happens via `riku init` itself, not auto-run by the installer).
- ✅ **`riku quickstart`** — scaffolds a sample app and prints the exact `git remote add` line so a new user can deploy in under five minutes.
- ✅ **Better first-deploy output** — `git push` streams the deploy log line-by-line (`DeployLogger`, Heroku-style `-----> ` prefix), logs the detected runtime, and ends with a prominent `<app> deployed!` plus the live URL (from `NGINX_SERVER_NAME`) or a hint to add a domain.

### Phase 1 — Finish the Dashboard — **shipped**

- ✅ Two dashboards now exist: the embedded single-binary HTML dashboard
  (`riku-dashboard` crate, UI baked in via `include_str!` — no separate web
  stack, no Node runtime on the host) and a fuller Next.js/React browser
  dashboard (`dashboard/`, built/deployed separately) with app list, live
  log streaming, deploy history, env editor, addon management, and a
  plugin marketplace UI.
- ✅ Mutating actions (deploy/restart/stop, addon provision/bind/unbind,
  plugin install) are live on both — gated by the backend's operator
  token (`RIKU_DASHBOARD_TOKEN`) and, on the Next.js dashboard, a real
  frontend login: a single shared password (scrypt-hashed,
  `RIKU_DASHBOARD_PASSWORD_HASH`), signed session cookie, enforced by
  middleware on every route. Auth is a no-op when the password hash is
  unset (matches Riku's "don't break default/local usage" default).
- **Still open**: a pluggable auth-*provider* seam (e.g. GitHub/SSO login
  to the dashboard as a plugin) — the dashboard has a real login now, but
  not one a plugin can supply or replace. See the "Auth / SSO" row below.

### Phase 2 — Trust & Resilience — **shipped**

What makes a solo dev put a *real* project on Riku.

- ✅ **Backups** — `riku backup <app>` / `riku restore <app>` (tar-based; S3/remote-storage output is still open).
- ✅ **Rollback** — `riku rollback <app>`, atomic symlink swap under the per-app deploy lock.
- ✅ **Zero-downtime deploys** — canary/generation-based health-gated cutover (`riku-supervisor::process::generation`/`orchestration`).
- ✅ **`riku doctor`** — diagnoses nginx/systemd/permissions/disk/cert state.

### Phase 3 — Stateful Apps — **shipped**

The biggest single unblock for solo devs — shipped as plugins (see Track B), not core bloat.

- ✅ **Managed datastores as addons** — Postgres, Redis, SQLite-volume ship
  as example bundles in this repo's own starter marketplace
  (`examples/plugins/{postgres,redis,sqlite-volume}`,
  `riku plugins add postgres`). Not bundled/installed by default the way
  runtime plugins are — an explicit install step, by design (addons hold
  credentials and are the highest-trust seam).
- ✅ `bind` injects the addon's returned env (e.g. `DATABASE_URL`) into the
  app; `unbind` removes exactly those keys.

---

## Track B — Plugin Ecosystem

The original plugin surface was thin: four runtime verbs (`detect` /
`build` / `env` / `start`) plus four fixed lifecycle hooks (`pre-deploy`,
`pre-build`, `post-build`, `post-deploy`). It has since grown considerably
(below) — the remaining gap toward "vast ecosystem" is **distribution**
(marketplace, E2) and **untrusted-author sandboxing** (WASM, E3), not
plugin-type breadth, which is now largely closed.

### Phase E0 — Stabilize & Document the Contract — **shipped**

- ✅ **Plugin protocol versioned** — `RIKU_PLUGIN_API=1`, published and
  actively maintained in `PLUGIN_PROTOCOL.md` (verb I/O, capabilities,
  every seam and event listed below).
- ✅ **`riku plugins scaffold <name>`** — generates a bundle skeleton
  (`--type runtime|addon|notifier`).

### Phase E1 — Expand Plugin Types — **shipped**

Every category from the original table now exists, plus several the
original table didn't anticipate:

| Plugin type              | Contract (verbs)                                             | Status |
| ------------------------- | -------------------------------------------------------------- | ------ |
| Runtime                   | `detect` / `build` / `env` / `start`                            | shipped |
| Addon / Resource          | `provision` / `bind` / `unbind` / `deprovision` / `backup`      | shipped |
| Legacy hook                | `pre`/`post` `deploy`/`build` (fixed, 4 stages)                 | shipped (predates the event bus below) |
| Router                     | `configure` / `reload`                                          | shipped — host-level singleton (`RIKU_ROUTER=<name>`), swaps nginx entirely |
| Event subscriber            | `on_event` — `subscribe`/`mode`/`priority` in `[events]`        | shipped — `deploy.*`, `build.*`, `app.restarted`, `app.failed`; `riku-notify` (webhook/Discord/Slack/Telegram) is the shipped first-party example |
| Custom events               | `riku plugin-emit <plugin.custom.*>` (opt-in `events.emit`)     | shipped — namespaced, kernel-stamps `source_plugin` so a subscriber can tell a real kernel event from a plugin-claimed one |
| Filter (value-transform)   | `on_filter` — `subscribe`/`priority` in `[filters]`             | shipped — chains, always degrades to passthrough on failure; `nginx.include_content` (augments the generated nginx config) is the shipped example |
| Lifecycle hooks            | `on_install` / `on_uninstall` (opt-in `[lifecycle]`, any plugin type) | shipped |
| UI panel                   | `ui_panel` (opt-in `[ui]`, Next.js dashboard only)              | shipped — structured JSON only, dashboard renders it, no plugin-supplied HTML/JS |
| Auth / SSO                 | dashboard auth *provider* seam                                   | **not shipped** — the dashboard has real login (Phase 1 above), but not as something a plugin can supply/replace |

The **addon contract is (still) the keystone** — it is how managed
datastores (Track A, Phase 3) ship as plugins instead of bloating core.
It keeps the single-binary purity while delivering the ecosystem's
killer plugin category.

### Phase E2 — Distribution & Discovery (Claude-style marketplace) — **shipped**

What turns plugins into an *ecosystem*. The model is adapted directly from Claude Code's plugin/marketplace design: **git-native, no central server, manifest-indexed, multi-marketplace, namespaced installs.** Riku copies the distribution UX and layers a stricter server-side trust model on top (see "Plugin Trust Model" below) — because a Riku plugin is an executable that runs on your server, not an instruction run in a local client.

All of the below is shipped, including a real starter marketplace: this
repo's own `marketplace.toml` indexes `examples/plugins/{sqlite-volume,
postgres, redis, caddy-router, webhook-notify}` — `riku plugins
marketplace add github:dreygur/riku` registers it directly.

**Bundle layout** — a plugin is a directory (git repo or subdir), not a single file:

```
my-plugin/
  riku-plugin.toml      # manifest
  bin/                  # executable(s) implementing the type's verbs
  README.md
```

**Manifest** (`riku-plugin.toml`):

```toml
name        = "postgres"
version     = "1.2.0"
type        = "addon"            # runtime | addon | hook | router | notifier | auth
api         = 1                  # RIKU_PLUGIN_API this plugin targets
entry       = "bin/riku-postgres"
checksum    = "sha256:..."       # verified on install
description = "Managed PostgreSQL addon"
author      = "..."

[capabilities]                   # declared, shown on install, enforced where possible
network     = true
writes      = ["app_dir", "data_dir"]
privileged  = false
```

**Marketplace** — a git repo with an index listing plugins and their `source`. No server; a GitHub repo *is* the marketplace:

```toml
# marketplace.toml
[[plugin]]
name        = "postgres"
source      = "github:riku-plugins/postgres"
description = "Managed PostgreSQL addon"
type        = "addon"
```

**CLI** (mirrors the Claude `marketplace add` → `install name@marketplace` flow):

- `riku plugins marketplace add <git-url>` — register a marketplace. Warns that this lets the marketplace publish code that runs on the server; first-party marketplace trusted by default, third-party opt-in.
- `riku plugins marketplace list / remove`
- `riku plugins search <query>` — reads **manifests only** (progressive disclosure; payload pulled on install).
- `riku plugins add <name>@<marketplace>[@<version>]` — install, namespaced + version-pinnable.
- `riku plugins remove <name>` (no `update` yet — see Phase E3: currently remove + reinstall)
- `riku plugins add ./path` — install from local path for the authoring/dev loop.
- **Lockfile** (`riku-plugins.lock`) — pins resolved name + marketplace + version + checksum. No silent auto-update of executable code.

**Official starter marketplace** — shipped (see above). Runtimes beyond
node/python/ruby/go/rust-lang/java/clojure/container (e.g. php, elixir,
deno, bun) are still open if demand shows up.

### Phase E2.5 — Plugin Trust Model — **shipped**

Riku plugins run **on the server, as the deploy user, with filesystem and network access** — a far larger blast radius than a Claude skill run in a local client. So Riku copies Claude's distribution UX but hardens the security. All of the below is shipped:

- ✅ **Checksum + signature verification on install** — a pinned `sha256` mismatch is rejected; an Ed25519 `signature` (`riku plugins keygen`/`sign`) must verify against a key the operator explicitly trusts (`riku plugins trust add/list/remove`) or the install is rejected outright.
- ✅ **Pinned versions + lockfile** (`riku-plugins.lock`) — records name, source, version, checksum, and verifying key; no silent auto-update of executable code.
- ✅ **Explicit trust on `marketplace add`** — third-party marketplaces are opt-in.
- ✅ **Capability declaration** (`network`/`writes`/`privileged`) — shown on install, enforced at spawn time via Landlock + `no_new_privs` wherever the kernel supports it.
- **WASM sandbox** for untrusted-author plugins — see Phase E3. **Still open** — a "vast ecosystem" means "lots of untrusted code," so sandboxing is the long-term answer, not an afterthought.

Riku deliberately does **not** copy the looser "add a marketplace and run executables" posture wholesale — that is a supply-chain footgun on a server rather than a local dev tool.

### Phase E3 — Ecosystem Growth

- ✅ **`riku plugins doctor`** — validates installed plugins against the current API version and re-checks integrity against the lockfile.
- **Still open**: a plugin docs site + gallery (beyond `docs/docs/plugin-bundles.md` and `docs/docs/plugin-gallery.md`, which already exist but aren't a searchable gallery), and `riku plugins update <name>` (currently: remove + reinstall).
- **WASM plugin option** (optional, later) — sandboxed plugins for untrusted authors, to keep the security model tight as the ecosystem grows. **Still open.**

---

## Sequencing

Honest priority order across both tracks (✅ = shipped since this was last written):

1. ✅ Installer + `quickstart` + first-deploy output — cheap, unblocks *all* adoption. All shipped together in `41bb349`.
2. ✅ Dashboard, mutating actions included, both embedded and Next.js.
3. ✅ Plugin contract v1 + `scaffold`.
4. ✅ Addon seam + Postgres/Redis/SQLite-volume as installable example addons (not bundled by default — an explicit `riku plugins add`, since addons hold credentials).
5. ✅ Backups + rollback + zero-downtime cutover + `doctor`.
6. ✅ Marketplace (`riku plugins marketplace add/list/remove`, `search`, `add name@market`) + lockfile + checksum/signature verification + capability declaration, shipped together from the start.
7. ✅ Notifier / router / filter / custom-event / UI-panel plugins. A searchable docs gallery beyond the existing reference pages is still open.
8. ✅ Dashboard mutating actions. WASM plugin sandbox and a dashboard auth-*provider* seam (SSO as something a plugin supplies) — **still open.**

Given the above, every numbered item on this list has shipped except the
open ends already called out inline: the docs gallery (#7), and the WASM
sandbox plus dashboard auth-provider seam (#8).

---

## Milestones & Effort

Estimates are for **one experienced Rust developer who already knows this codebase**, at MVP quality (working and tested, not gold-plated). Ranges reflect uncertainty. "Dev-weeks" = full-time-equivalent effort, not calendar time.

> **Calendar conversion:** a solo maintainer working a side-project at roughly part-time (~10 hr/week) runs at about a quarter of full-time, so multiply dev-weeks by ~4 for realistic calendar time.

| Phase | Scope | Dev-weeks | Risk |
| ----- | ----- | --------- | ---- |
| 0 — Installer / quickstart | one-line installer, `quickstart`, first-deploy output | ✅ shipped | — |
| 1 — Dashboard | app list, live log stream, history, env editor, mutating actions, auth | ✅ shipped | — |
| 2 — Trust & resilience | backups/restore, rollback, zero-downtime cutover, `doctor` | ✅ shipped | — |
| E0 — Contract v1 | protocol version, spec, scaffold | ✅ shipped | — |
| E1 — Plugin types | addon/router/event/filter/custom-event/UI-panel/lifecycle dispatch | ✅ shipped (auth-provider seam still open) | — |
| 3 — Postgres addon | first managed datastore | ✅ shipped (example bundle) | — |
| E2 — Marketplace | git fetch, manifest, search, install, lockfile, checksum | ✅ shipped | — |
| E2.5 — Trust model | signature verify, capability enforcement | ✅ shipped | — |
| E3 — Docs + `plugins doctor` | gallery, validation | `doctor` ✅ shipped; searchable gallery still open | low |
| E3 — WASM sandbox | wasmtime + host API + port plugin model | 6–10 | high |

### What's left, honestly

Every phase above is now shipped except the WASM sandbox (part of E3).
The MVP-slice/full-roadmap effort math below is kept for historical
context (it's what this roadmap estimated before that work happened) —
treat it as a record, not a live estimate. The one remaining item:

- **WASM sandbox (E3)** — deliberately deferred; the Landlock-based
  capability enforcement shipped in E2.5 covers the "well-behaved
  first-party plugin" case, but not a fully untrusted third-party author.
  Build this when ecosystem size actually demands it, not before.

<details>
<summary>Original effort estimates (historical — most of this has since shipped)</summary>

#### MVP slice (as originally scoped)

The smallest set that was judged to actually move adoption:
**installer + read-only dashboard + the addon contract + a working Postgres addon.**

- Phases: **0 + 1 + E0 + E1 (addon only) + 3**
- Effort: **~12–18 dev-weeks ≈ 3–4 months full-time** (≈ 9–12 months part-time).
- Outcome: a new user installs in one line, deploys via `git push`, sees apps and live logs in a browser, and attaches a managed Postgres. That is the "I'd run my side-project on this" threshold.

#### Full roadmap (as originally scoped)

- **Core (everything except the WASM sandbox):** ~22–34 dev-weeks, plus ~30% for integration/testing/docs → **~30–44 dev-weeks ≈ 7–10 months full-time** (≈ 1.5–2.5 years part-time).
- **With the WASM sandbox:** add ~2–2.5 months → **~9–12 months full-time** (≈ ~3 years part-time).

#### Estimate caveats (as originally written)

- The three high-risk items — dashboard live-log streaming, plugin **capability enforcement** (real Linux sandboxing without containers is genuinely hard — the very thing Riku avoids), and the **WASM sandbox** — carry most of the schedule risk and could each run ~2x over.
- Estimates assume no major scope creep and a single contributor who knows the code. More contributors help on parallel tracks (DX vs ecosystem) but add coordination cost.

</details>

---

## Guiding Principles

- **Do step 1 before everything.** The installer, `quickstart`, and first-deploy output all shipped in `41bb349` — this held even before the rest of the roadmap was built out, since losing users at minute one erases everything built downstream of it.
- **The Addon plugin contract is the strategic core.** It lets the *plugin system* deliver databases, so core stays single-binary while the ecosystem gains its killer category — both goals served by one design.
- **A marketplace is what "vast ecosystem" actually means.** Extensible is not the same as an ecosystem; discovery plus one-command install is the difference. Adopt Claude Code's proven git-native marketplace shape rather than inventing one — but harden it for server-side execution.
- **Freeze a small, stable API rather than chasing a rich, unstable one.** A stable small contract grows more third-party plugins than a sprawling unstable one. Pick the plugin types above, version them, and hold the line.
- **Every phase preserves the identity:** single binary, no Docker required, runs on one small box. Addons and databases ship as plugins; the primary dashboard is embedded in the binary. (The separate, fuller Next.js dashboard is optional and deployed independently — it does not compromise the core binary's zero-dependency identity, since riku runs and is fully usable without it.)

---

## Out of Scope

Riku is not trying to be Coolify. The following are deliberately **not** on the roadmap, because they break the single-box / single-binary identity and put Riku into a fight it cannot win against funded, dashboard-first platforms:

- Multi-server / cluster orchestration.
- Container orchestration as a core requirement.
- Multi-tenant teams / RBAC as a core concern (an SSO plugin is the ceiling).
- A heavy external datastore for platform state.

Riku competes with Piku, Dokku, and CapRover — and wins on the Rust single-binary, no-Docker story. The roadmap leans into that, not away from it.

# Riku Roadmap

Two goals drive this roadmap:

1. **Solo-dev friendly** — make Riku the nicest single-box, no-Docker, git-push PaaS for an individual developer.
2. **Vast plugin ecosystem** — turn Riku's extension points into a real ecosystem with a stable contract, many plugin types, and one-command discovery/install.

Both goals respect Riku's identity: a single Rust binary, no runtime dependencies, no Docker required, runs on one small box (VPS / SBC / homelab).

This is a living document. Phases are ordered by leverage, not by size.

---

## Track A — Solo-Dev DX (adoption funnel)

### Phase 0 — First-5-Minutes Magic (highest leverage)

- **One-line installer** — `curl -sSL get.riku.sh | sh`. Detects OS, drops the binary, runs `riku init`, configures systemd + nginx. No installer exists today; this is the single biggest adoption gap.
- **`riku quickstart`** — scaffolds a sample app and prints the exact `git remote add` line so a new user can deploy in under five minutes.
- **Better first-deploy output** — on `git push`: stream the build log, show the detected runtime, and print the final URL prominently. Make the "it works" moment unmissable.

### Phase 1 — Finish the Dashboard (read-first)

- Land the `feat/dashboard-control-plane` work: app list, status, **live logs in the browser**, deploy history, env editor.
- Ship read-only first (safe), then add mutating actions (restart / scale / redeploy) behind the existing operator-token + CSRF protection.
- Embed UI assets **into the binary** (e.g. `rust-embed`) to preserve the single-binary identity. No separate web stack, no Node runtime on the host.

### Phase 2 — Trust & Resilience

What makes a solo dev put a *real* project on Riku.

- **Backups** — `riku backup <app>` / `riku restore <app>`: app source + env + volumes to tar / S3. Cron-able.
- **Rollback** — `riku rollback <app>`: keep N releases, atomic symlink swap (the per-app deploy lock already protects the critical section).
- **Zero-downtime deploys** — health-gated cutover wired into the deploy path (the health subsystem already exists).
- **`riku doctor`** — diagnose nginx / systemd / permissions / disk / cert state. Solo devs have no ops team; the tool *is* the ops team.

### Phase 3 — Stateful Apps

The biggest single unblock for solo devs — shipped as plugins (see Track B), not core bloat.

- **Managed datastores as addons** — Postgres, Redis, SQLite-volume.
- Auto-inject `DATABASE_URL` / `REDIS_URL` into app env on bind. This converts Riku from "toy" to "I run my SaaS side-project on it."

---

## Track B — Plugin Ecosystem

Today's plugin surface is too thin for an ecosystem: four runtime verbs (`detect` / `build` / `env` / `start`) plus four lifecycle hooks (`pre-deploy`, `pre-build`, `post-build`, `post-deploy`). Breadth requires more plugin **types**, easy **distribution**, and stable **contracts**.

### Phase E0 — Stabilize & Document the Contract

- **Version the plugin protocol** — `RIKU_PLUGIN_API=1`. Without a stable contract, nobody invests in building plugins.
- **Publish a spec** — document each verb plus a JSON I/O schema. Today it is argv + env vars + line-parsing; formalize it.
- **`riku plugins scaffold <name>`** — generate a working plugin skeleton (shell and Rust-crate variants). Lower authoring cost means more authors.

### Phase E1 — Expand Plugin Types

The breadth unlock. Add categories beyond buildpacks:

| Plugin type        | Contract (verbs)                                  | Unlocks                          |
| ------------------ | ------------------------------------------------- | -------------------------------- |
| Runtime (exists)   | `detect` / `build` / `env` / `start`              | languages                        |
| Addon / Resource   | `provision` / `bind` / `unbind` / `deprovision` / `backup` | databases, caches, queues |
| Hook (exists)      | `pre`/`post` `deploy`/`build`                     | notify, migrate, warm cache      |
| Router             | `configure` / `reload`                            | swap nginx for Caddy / Traefik   |
| Notifier           | `on_event(json)`                                  | Slack / Discord / webhook        |
| Auth / SSO         | dashboard auth provider                           | GitHub login to the dashboard    |

The **Addon contract is the keystone** — it is how managed datastores (Track A, Phase 3) ship as plugins instead of bloating core. It keeps the single-binary purity while delivering the ecosystem's killer plugin category.

### Phase E2 — Distribution & Discovery (Claude-style marketplace)

What turns plugins into an *ecosystem*. The model is adapted directly from Claude Code's plugin/marketplace design: **git-native, no central server, manifest-indexed, multi-marketplace, namespaced installs.** Riku copies the distribution UX and layers a stricter server-side trust model on top (see "Plugin Trust Model" below) — because a Riku plugin is an executable that runs on your server, not an instruction run in a local client.

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
- `riku plugins remove / update <name>`
- `riku plugins add ./path` — install from local path for the authoring/dev loop.
- **Lockfile** (`riku-plugins.lock`) — pins resolved name + marketplace + version + checksum. No silent auto-update of executable code.

**Official starter marketplace** maintained in-repo — postgres, redis, slack-notify, caddy-router, and php / elixir / deno / bun runtimes. Seed the ecosystem so it does not look empty.

### Phase E2.5 — Plugin Trust Model

Riku plugins run **on the server, as the deploy user, with filesystem and network access** — a far larger blast radius than a Claude skill run in a local client. So Riku copies Claude's distribution UX but hardens the security:

- **Checksum + signature verification on install** — the manifest pins a `sha256`; reject on mismatch. Reuse the existing `subtle` / `sha` / `hex` stack. Optional author signature (e.g. minisign) for first-party + verified publishers.
- **Pinned versions + lockfile** — `name@market@1.2.0`; no silent upgrade of code that runs on the host.
- **Explicit trust on `marketplace add`** — loud warning; third-party marketplaces are opt-in, never auto-trusted.
- **Capability declaration** — manifest declares network / FS-write / privileged needs; shown on install (Android-permission style), enforced where the platform allows.
- **WASM sandbox** for untrusted-author plugins — see Phase E3. A "vast ecosystem" means "lots of untrusted code," so sandboxing is the long-term answer, not an afterthought.

Riku deliberately does **not** copy the looser "add a marketplace and run executables" posture wholesale — that is a supply-chain footgun on a server rather than a local dev tool.

### Phase E3 — Ecosystem Growth

- **Plugin docs site + gallery** — extend the existing mkdocs site.
- **`riku plugins doctor`** — validate installed plugins against the current API version.
- **WASM plugin option** (optional, later) — sandboxed plugins for untrusted authors, to keep the security model tight as the ecosystem grows.

---

## Sequencing

Honest priority order across both tracks:

1. Installer + `quickstart` — cheap, unblocks *all* adoption.
2. Dashboard, read-only — the branch is already started.
3. Plugin contract v1 + scaffold — prerequisite for the ecosystem.
4. Addon plugin type + Postgres — biggest solo-dev unblock; validates the addon model.
5. Backups + rollback + `doctor` — the trust tier.
6. Claude-style marketplace + `plugins add name@market` + lockfile — ecosystem ignition. Ship with checksum/signature verification and capability declaration from day one (do not bolt security on later).
7. Notifier / router plugins + gallery.
8. Dashboard mutating actions, WASM plugin sandbox, SSO.

---

## Milestones & Effort

Estimates are for **one experienced Rust developer who already knows this codebase**, at MVP quality (working and tested, not gold-plated). Ranges reflect uncertainty. "Dev-weeks" = full-time-equivalent effort, not calendar time.

> **Calendar conversion:** a solo maintainer working a side-project at roughly part-time (~10 hr/week) runs at about a quarter of full-time, so multiply dev-weeks by ~4 for realistic calendar time.

| Phase | Scope | Dev-weeks | Risk |
| ----- | ----- | --------- | ---- |
| 0 — Installer / quickstart | one-line installer, `quickstart`, first-deploy output | 2–3 | low |
| 1 — Dashboard (read-only) | app list, live log stream, history, env editor, embedded assets | 3–5 | med (live logs) |
| 2 — Trust & resilience | backups/restore, rollback, zero-downtime cutover, `doctor` | 3–5 | med |
| E0 — Contract v1 | protocol version, spec, scaffold | 1.5–2 | low |
| E1 — Plugin types | core dispatch for addon/router/notifier/auth + lifecycle wiring | 4–6 | med (addon ~2 alone) |
| 3 — Postgres addon | first managed datastore, once E1 lands | 1.5–2 | low |
| E2 — Marketplace | git fetch, manifest, search, install, lockfile, checksum | 3–5 | med |
| E2.5 — Trust model | signature verify, capability enforcement | 2–4 | high (enforcement) |
| E3 — Docs + `plugins doctor` | gallery, validation | 1.5–2 | low |
| E3 — WASM sandbox | wasmtime + host API + port plugin model | 6–10 | high |

### MVP slice (ship this first)

The smallest set that actually moves adoption: **installer + read-only dashboard + the addon contract + a working Postgres addon.**

- Phases: **0 + 1 + E0 + E1 (addon only) + 3**
- Effort: **~12–18 dev-weeks ≈ 3–4 months full-time** (≈ 9–12 months part-time).
- Outcome: a new user installs in one line, deploys via `git push`, sees apps and live logs in a browser, and attaches a managed Postgres. That is the "I'd run my side-project on this" threshold.

### Full roadmap

- **Core (everything except the WASM sandbox):** ~22–34 dev-weeks, plus ~30% for integration/testing/docs → **~30–44 dev-weeks ≈ 7–10 months full-time** (≈ 1.5–2.5 years part-time).
- **With the WASM sandbox:** add ~2–2.5 months → **~9–12 months full-time** (≈ ~3 years part-time).

### Estimate caveats

- The three high-risk items — dashboard live-log streaming, plugin **capability enforcement** (real Linux sandboxing without containers is genuinely hard — the very thing Riku avoids), and the **WASM sandbox** — carry most of the schedule risk and could each run ~2x over.
- Estimates assume no major scope creep and a single contributor who knows the code. More contributors help on parallel tracks (DX vs ecosystem) but add coordination cost.
- Do not plan to "complete the roadmap." Ship the MVP slice, get real users, and let usage reorder E2 / E2.5 / E3. The WASM sandbox is the first thing to defer if time-boxed — build it when ecosystem size actually demands untrusted-author isolation, not before.

---

## Guiding Principles

- **Do step 1 before everything.** No installer means losing users at minute one, regardless of features.
- **The Addon plugin contract is the strategic core.** It lets the *plugin system* deliver databases, so core stays single-binary while the ecosystem gains its killer category — both goals served by one design.
- **A marketplace is what "vast ecosystem" actually means.** Extensible is not the same as an ecosystem; discovery plus one-command install is the difference. Adopt Claude Code's proven git-native marketplace shape rather than inventing one — but harden it for server-side execution.
- **Freeze a small, stable API rather than chasing a rich, unstable one.** A stable small contract grows more third-party plugins than a sprawling unstable one. Pick the plugin types above, version them, and hold the line.
- **Every phase preserves the identity:** single binary, no Docker required, runs on one small box. Addons and databases ship as plugins; UI is embedded in the binary.

---

## Out of Scope

Riku is not trying to be Coolify. The following are deliberately **not** on the roadmap, because they break the single-box / single-binary identity and put Riku into a fight it cannot win against funded, dashboard-first platforms:

- Multi-server / cluster orchestration.
- Container orchestration as a core requirement.
- Multi-tenant teams / RBAC as a core concern (an SSO plugin is the ceiling).
- A heavy external datastore for platform state.

Riku competes with Piku, Dokku, and CapRover — and wins on the Rust single-binary, no-Docker story. The roadmap leans into that, not away from it.

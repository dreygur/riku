# Design Prompt — Riku Control Dashboard (v2)

> Paste this to Claude to design and build the new dashboard. It ships with
> `FEATURE_INVENTORY.md` (the full capability + API map) — treat that as the
> source of truth for what the UI must cover.

---

## Role

You are the design lead and front-end engineer rebuilding the web dashboard for
**Riku** — a single-binary micro-PaaS that gives solo developers Heroku-style
`git push` deploys on one small box (a VPS, an old laptop, a Raspberry Pi). The
operator is usually SSH-range close to their own server, watching a handful of
their own apps. The dashboard's job: **see every app's health at a glance, and
drive every riku capability without touching the CLI.**

The current dashboard is thin and generic. Replace it with something that feels
purpose-built for a machine you operate — a control deck, not a SaaS console.

## Stack (hard requirements)

- **Next.js** (latest, App Router, React Server Components where sensible).
- **TailwindCSS latest (v4)** — CSS-first config, `@theme` tokens.
- **shadcn/ui** for the component layer (latest CLI/registry).
- **animate-ui** (https://animate-ui.com) for motion — use its components/primitives
  for transitions, reveals, and micro-interactions rather than hand-rolling.
- Keep the existing **Next API proxy** pattern that deputizes the riku control
  token server-side (the browser never sees `~/.riku/control.token`).

## Non-negotiable visual constraints

1. **Nothing is rounded.** Zero border-radius, everywhere. Set shadcn's
   `--radius: 0rem` and Tailwind's radius scale to `0`; square every card,
   button, input, badge, popover, dialog, avatar, and chart element. Sharp
   corners are the spine of the look.
2. **Dark / darkish theme** as the default and primary (a light mode is optional,
   not required). Deep, slightly-warm or slightly-cool near-black — not pure
   `#000`, not the generic acid-green-on-black. Pick a deliberate palette.
3. **Restraint:** one accent color used only to mean something (alive / action).
   Status colors (ok / warn / danger) appear only where they carry meaning.

## Aesthetic direction (take it further than this if you have a better idea)

The subject's world is the **terminal and the bare-metal box**. Lean into a
*mission-console / machine-readout* aesthetic: dense, aligned, monospace for all
machine data (pids, mem, cpu, SHAs, timestamps, log lines), a clean sans for
chrome and prose. Hairline dividers, a tight grid, square everything. Numbers
are first-class — the page is a readout. Avoid the three AI defaults
(cream+serif+terracotta; near-black+acid-green; broadsheet columns); if you land
near one, change it and say why.

Pick one **signature element** and spend your boldness there (e.g. a live status
strip that pulses with each app's heartbeat, a command-palette that drives every
riku action, or a console drawer that feels like a real tty). Keep everything
around it quiet.

## Information architecture (must cover all of FEATURE_INVENTORY.md)

- **Overview** — every app as a status row/card: state, domain, nginx state,
  worker health, mem/cpu at a glance, deploy-lock ("deploying…"). Host header:
  riku version, supervisor uptime, app count, live indicator.
- **App detail** — workers table (kind.ordinal, status, pid, mem, cpu, restarts),
  actions (redeploy / restart / stop / destroy / scale / rollback / export image),
  routing panel (domain, https, static, cache, cloudflare), env editor,
  release timeline (roll back to any SHA), backups.
- **Live logs** — per-app and per-worker, streamed over SSE; pause, follow,
  filter by worker, search. Make it feel like a console.
- **Metrics** — cpu / mem / requests over time (SSE `/metrics/stream`),
  per-worker and aggregated; host gauges.
- **Plugins & addons** — installed runtime/hook plugins; managed datastores
  (addons: create / bind / unbind / backup / destroy); install bundled runtimes.
- **Marketplace** — browse, search, add sources, install/remove bundles; trust
  keyring (author signatures); plugins doctor.
- **System** — `riku doctor` results (nginx/systemd/perms/disk/cert), health.
- **Command palette (⌘K)** — fuzzy-run any action on any app; the power-user spine.

Not every panel needs a backend endpoint yet — `FEATURE_INVENTORY.md` §10 lists
what's CLI-only. Design the surface fully; stub or disable actions whose endpoint
doesn't exist, with a clear "not wired yet" affordance, so the IA is complete and
the backend can fill in behind it.

## API contract (summary — full map in FEATURE_INVENTORY.md)

- **Read (open on loopback):** `GET /api/state`, `/metrics`, `/metrics/apps[/:app]`,
  `/metrics/stream` (SSE), `/plugins`, `/hooks`, `/api/apps/:app/releases`,
  `/api/apps/:app/logs` (SSE).
- **Mutating (token + same-origin/CSRF, via the Next proxy):**
  `POST /control/apps` (create), `/control/apps/:app/{deploy,restart,stop}`,
  `DELETE /control/apps/:app`, `/control/plugins/install`,
  `/control/apps/:app/container/export`, plus `scale` and `rollback`.
- **Wire details:** process status is **snake_case** (`running`, `starting`,
  `stopped`, `crashed`, `restarting`, `oom_killed`); health is snake_case too.
  Worker entries carry `memory_bytes`, `cpu_time_ms`, `restart_count`, `pid`.

## Interaction & quality

- **Optimistic, reversible actions** with toasts in the interface's voice
  ("Restarting web…" → "Restarted web"; errors say what failed and how to fix).
- **Live first:** logs and metrics stream; the overview refreshes without a full
  reload; the live indicator reflects real connection state.
- **Guarded destructive actions** (stop / destroy / rollback) — confirm, name the
  consequence, never a bare "Submit".
- **Empty + error states are directions, not mood** ("No apps yet — push one:
  `git push riku main`").
- **animate-ui** for: page/section transitions, list-item reveals on load, the
  log/console drawer, status-dot heartbeat, command-palette open. Orchestrated,
  not scattered. Respect `prefers-reduced-motion`.
- **Quality floor:** responsive to mobile, visible keyboard focus, real contrast,
  keyboard-drivable (palette + tab order). Don't announce it — just meet it.

## Copy

Write from the operator's side of the screen. Name things by what they control
("Environment", "Workers", "Releases"), not by how riku is built. Active voice on
every control; the verb on the button is the verb in the toast. Sentence case,
plain, specific.

## Deliverable

A working Next.js + Tailwind v4 + shadcn (radius 0) dashboard with animate-ui
motion, dark by default, covering the IA above against the real API, with stubs
for the not-yet-wired actions. Show a short design plan first (palette as named
hex, the display/body/mono type pairing, layout concept, and the one signature
element) and confirm it isn't a generic default before building.

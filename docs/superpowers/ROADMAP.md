# Roadmap and resume point

Read this first in a fresh session. It says what is done, what runs next and
where each decision lives. Update the status lines in the same commit as the
work they describe.

## How work runs here

- Each project has a design in `docs/superpowers/specs/` (owner approved) and
  a plan in `docs/superpowers/plans/`, written just before it runs.
- Plans run with superpowers:subagent-driven-development: one implementer
  subagent per task, a review after each, a final whole branch review.
- The run ledger lives in `.superpowers/sdd/<plan>/progress.md` (git ignored,
  this machine only). The committed record is git history plus the status
  lines below: a task is done when its commit is listed here.
- Owner rules for agents: no python, perl or other script interpreters for
  reading or editing files; no AI attribution in commits; follow
  `.claude/CLAUDE.md`.

## Status

### Done

- Remote CLI and redesigned embedded dashboard. Spec
  `specs/2026-10-29-remote-cli-design.md`, plan
  `plans/2026-10-29-remote-cli.md`. Merged into `feat/caddy-router` at
  e8addf6, pushed, PR #71 ("Caddy router, remote CLI and dashboard
  redesign"). Host checked in a Docker test host; not yet deployed on
  edge (owner will do it later).

### Phase 1: rk client (done)

- Spec `specs/2026-10-30-rk-client-design.md`, plan
  `plans/2026-10-30-rk-client.md`. Merged into `feat/caddy-router` at
  4b120b0, not pushed.
- Task 1 riku-args: done (79cf37b)
- Task 2 riku-remote: done (52bd626)
- Task 3 rk binary: done (f662519)
- Task 4 packaging and docs: done (73096cf)
- Final review fixes: 14c2cce..1094a23. rk runs plugins scaffold, keygen
  and sign locally through the riku-bundle crate: 3b3e068..4b120b0.
- Host check on edge.example, 2026-10-30, server riku built from
  4b120b0: `rk -s edge apps`, `run myapp echo "a b"` prints
  `a b` (after `riku doctor --fix` migrated the old forced command keys),
  `dashboard` through the admin login, refusals, plugin authoring. mkdocs
  strict build passes. The supervisor there still runs the old binary
  until `riku.service` restarts.

### Phase 2: container services foundations (after phase 1)

- Spec `specs/2026-10-30-container-services-design.md` (owner decisions
  recorded, no open questions). Plan not written yet: write it after phase 1
  lands, because phase 1 moves the files it would name.
- Planned as four parallel tracks in separate worktrees after one small shared
  types task: A engine driver, B compose reader, C routing plan for nginx,
  Caddy and router plugins, D riku file sync.

### Phase 3: container integration (after phase 2)

- Same spec. Supervisor runs containers, rolling replacement, compose
  migration, per copy ports for Procfile apps, state API and dashboard rows,
  CLI scale and logs, `rk deploy --build`. Mostly sequential; dashboard and
  CLI tasks in parallel once the state API exists.

### Phase 4: one rk managing many servers

- No design yet. The owner wants to be asked questions interactively first.

## Known open problems, outside the plans

- `riku init` decides root from `$USER`, not the effective uid
  (crates/riku-cli/src/setup/init.rs around line 202).
- `riku install-plugins` failed to download inside a Docker test container
  (reqwest); cause not confirmed.
- The agent key form `command="riku agent --scope ..."` in
  `docs/docs/ai-agents.md` does not parse.
- Bracketed IPv6 server addresses are untested end to end.
- A dropped link during `riku dashboard -s` reports "the remote dashboard
  ended (exit status: 255)" rather than saying the link dropped.

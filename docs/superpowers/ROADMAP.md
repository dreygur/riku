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
- Implementers and reviewers run on Claude (Sonnet by default, Opus for
  reviews of trust boundaries and the final review), not through agy, which
  was too slow here.
- The owner's real servers, organisation and projects are confidential. The
  repository is public. Never write their names in code, tests, docs, commit
  messages or PR text; use neutral names (`edge`, `prod`, `edge.example`,
  `myapp`). Tell every subagent this rule, and search the diff before every
  commit and push. Ask the owner for the names to search for; they are not
  written down anywhere.

## Status

### Done

- Remote CLI and redesigned embedded dashboard. Spec
  `specs/2026-10-29-remote-cli-design.md`, plan
  `plans/2026-10-29-remote-cli.md`. Merged into `feat/caddy-router` at
  e8addf6, pushed, PR #71 ("Caddy router, remote CLI and dashboard
  redesign"). Host checked in a Docker test host; not yet deployed on
  the owner's test server (owner will do it later).

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
- Host check on the owner's test server, 2026-10-30, server riku built
  from 4b120b0: `rk -s <server> apps`, `run myapp echo "a b"` prints
  `a b` (after `riku doctor --fix` migrated the old forced command keys),
  `dashboard` through the admin login, refusals, plugin authoring. mkdocs
  strict build passes. The supervisor there still runs the old binary
  until `riku.service` restarts.

### Phase 2: container services foundations (done)

- Spec `specs/2026-10-30-container-services-design.md`, plan
  `plans/2026-10-30-container-foundations.md` (owner answered its three
  open questions on 2026-10-30; recorded in its Global Constraints).
- How the owner wants it run (agreed 2026-10-30):
  1. First, as its own small fix on `feat/caddy-router`: confirm with a
     failing test, then fix, the suspected nginx bug:
     `templates/nginx_https_only.conf.tera:1-3` writes the upstream socket
     path without the `unix:` prefix, and
     `crates/riku-nginx/src/template.rs:107-117` picks the HTTPS only
     template before the port map, so an HTTPS only Procfile web app may
     proxy to the wrong upstream. Not yet checked against real nginx.
  2. Task 0 on a new branch `feat/container-foundations` off
     `feat/caddy-router`, reviewed before anything else starts.
  3. Tracks A, B, C, D at the same time, each in its own worktree from Task
     0; tasks inside a track in order, a review after each. Reviews: Opus
     for tracks A (engine argv) and D (sync receiver is a trust boundary),
     Sonnet for B, C and Task 0. At most 4 agents at a time.
  4. Merge the tracks into `feat/container-foundations` (only `Cargo.lock`
     should conflict), final whole branch review on Opus, gate.
  5. Ask the owner before merging into `feat/caddy-router`, pushing, or
     touching the test server (engine detection and a sync round trip into
     a temporary directory there).
- Step 1 nginx fix: done (cf89ff3). nginx 1.27 confirmed the HTTPS
  only template never rendered (`RIKU_NGINX_ROOT` unset), and its parts
  failed `nginx -t`: a bare socket upstream and a duplicate ACME location.
  It also ignored the port map and uWSGI. The wsgi template passed a bare
  socket to `uwsgi_pass`. All fixed; rendered configs pass `nginx -t`.
- Plan `plans/2026-10-30-container-foundations.md`. Task status:
- Task 0 Shared types: done (8a0abbd)

- A1 Engine detection and the engine setting: done (8efebee)
- A2 Engine identity and command probe: done (f0cf570..2d9fc18)
- A3 Run and control containers: done (6172f4c..cd76b46)
- A4 Container state and stats: done (438a55f..f5f8caa)

- B1 Move compose file lookup into the library: done (bfedb76)
- B2 Read compose YAML with interpolation and key checks: done (897836a)
- B3 Parse service fields: done (f2e6807..0a454d9)
- B4 Project rules: order, routing, scale, replacement, volume names: done
  (ad82fe5..456b37e)

- C1 RoutePlan and Caddy rendering: done (3a717f7)
- C2 nginx upstream from backends: done (ba61258..16d86b6)
- C3 Router plugin protocol 2: done (bd7753b)

- D1 Crate, wire types and manifest scan: done (f28b200..c47b5db)
- D2 Transfer plan, bundle and verified apply: done (a779210..6fe2177)
- D3 The sync conversation over a byte stream: done (0948ac6..a685219)

- Tracks merged at b22d010, 50cafa6, 44f0173, c747d31. Final review fixes
  c747d31..ad2b81c, residual fixes ad2b81c..34ccc44. Gate on 34ccc44: 1110
  passed, 0 failed, 2 ignored. Merged into `feat/caddy-router` at 10e956b
  (gate green there too), not pushed.

### Phase 3: container integration (next)

- Same spec, split in two plans (owner, 2026-10-31). 3a server side: plan
  `plans/2026-10-30-container-integration.md` (owner approved), 16 tasks,
  run like phase 2 on a branch `feat/container-integration` off
  `feat/caddy-router`. 3b `rk deploy --build` and riku-sync wiring: no plan
  yet, written after 3a. Owner decisions for 3a are in its Global
  Constraints. Ask the owner before pushing, merging elsewhere or touching
  the test server; ask for the confidential names before the first commit.
- 3a task status: none started.
- Docs tunnel fix (first small item): done.
- Phase 2 run ledger with every ruling, owner decision and about 60 deferred
  minor findings (several name phase 3 work):
  `.superpowers/sdd/2026-10-30-container-foundations/progress.md` (git
  ignored, this machine only). Read its "minor (deferred)", "Owner decided"
  and "Phase 3 note" lines while writing the plan.
- Duties from phase 2, each owned by a 3a task unless marked 3b:
- Compose migration duty from B4: a `Declared` project name (compose `name`,
  `COMPOSE_PROJECT_NAME`, the directory of a `COMPOSE_FILE` below the app
  root, or an app name that normalisation changes, such as `Shop` or `sh.op`)
  must be refused when it equals another riku app's name after both are
  normalised (`Shop`, `sh.op` and `shop` all compare as `shop`) or its
  existing volumes lack the matching compose project label, and only then may
  `ComposeProject::confirm_declared_name` be called.
- (3b) Sync receive runs under a process level deadline; a stalled client is killed
  so the app's sync lock is released.
- Before `run`, a container that already has the name but a different
  `riku.app` label is refused with a clear message (names can collide across
  apps: app `a-b` service `c` and app `a` service `b-c`).
- Compose migration removes compose's old containers by their compose project
  label before riku starts its own.
- The bind source symlink walk is repeated at container start.
- Each copy's host port is stored (as a label) so backends can be rebuilt
  after a supervisor restart.
- (3b) The sync size limit and free space margin (`ReceiveLimits`) are wired to a
  server setting.
- (3b) The ssh sync entry point logs the refusal cause `receive` returns.

### Phase 4: one rk managing many servers

- No design yet. The owner wants to be asked questions interactively first.

## Known open problems, outside the plans

- nginx: `RIKU_NGINX_CACHE_PREFIXES` puts `proxy_cache_path` inside the
  `server` block; nginx allows it only at `http` level, so `nginx -t` fails
  for any app that sets it.
- Certificates (`crates/riku-nginx/src/ssl.rs`): ACME issues per domain and
  stops at the first success, so other domains are uncovered; nginx serves
  the leaf `<app>.crt`, not `<app>.fullchain.crt`; `nginx_acme_firstrun`
  is registered but never selected. Needs a design.

- `riku install-plugins` failed to download inside a Docker test container
  (reqwest); cause not confirmed.
- The agent forced command `command="riku agent --scope ..."` now parses,
  but sshd runs it with the client's request only in `SSH_ORIGINAL_COMMAND`,
  which `riku agent` never reads, so an agent behind that key sees only the
  help output. Reading it needs a design, and tokens from the client must
  never override the forced command's `--scope`.

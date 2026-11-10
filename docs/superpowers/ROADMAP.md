# Roadmap and resume point

Read this first in a fresh session. It says what is done, what runs next and
where each decision lives. Update the status lines in the same commit as the
work they describe.

## How work runs here

- Each project has a design in `docs/superpowers/specs/` (owner approved) and
  a plan in `docs/superpowers/plans/`, written just before it runs.
- Plans run with superpowers:subagent-driven-development, cut for tokens
  (owner, 2026-11-07):
  - No review after each task. One whole branch review at the end, then
    one fix round. The owner reviews everything last.
  - Group small or related tasks into one implementer dispatch. Do work
    that takes a handful of tool calls inline, without a subagent.
  - Implementers run on Sonnet; mechanical work (deletions, renames,
    moving code) on Haiku. Opus only for the final review.
  - Briefs point at spec and plan sections and `file:line`; they do not
    paste code or retype the spec.
  - Per task, run only the touched crate's tests (`cargo test -p <crate>`).
    The full suite, clippy, the Linux run and Docker e2e run once, before
    the final review.
  - Plans stay short: task goal, files, acceptance check. No full code in
    the plan unless the change is subtle.
- The run ledger lives in `.superpowers/sdd/<plan>/progress.md` (git ignored,
  this machine only). The committed record is git history plus the status
  lines below: a task is done when its commit is listed here.
- Owner rules for agents: no python, perl or other script interpreters for
  reading or editing files; no AI attribution in commits; follow
  `.claude/CLAUDE.md`.
- Agents run on Claude, not through agy, which was too slow here.
- Order agreed 2026-11-09: developer experience (two small fixes, then the
  config model, then the first run), then small boards and binary size,
  then the nginx bugs (cache path, duplicate `location /`), then the
  framework design, then phase 4.
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

### Phase 3: container integration (3a done, PR #73 open)

- Same spec, split in two plans (owner, 2026-10-31). 3a server side: plan
  `plans/2026-10-30-container-integration.md` (owner approved), 16 tasks,
  run like phase 2 on a branch `feat/container-integration` off
  `feat/caddy-router`. 3b `rk deploy --build` and riku-sync wiring: no plan
  yet, written after 3a. Owner decisions for 3a are in its Global
  Constraints. Ask the owner before pushing, merging elsewhere or touching
  the test server; ask for the confidential names before the first commit.
- 3a status: all tasks done (2026-11-04), pushed, PR #73 against main,
  open. Later fixes on the same branch through ffd7909: real host checks 2
  to 5, @ naming, container and volume names, supervisor stops and pid
  files, `riku-sockets` group, `/etc/riku/caddy`, `/var/lib/riku` static
  and ACME roots, dashboard split, opt-level "z", push exit code. Ledger:
  `.superpowers/sdd/2026-10-30-container-integration/progress.md`.
- 3a task status (branch `feat/container-integration`):
  - Task 1 confirmed router reload: done (7673cc9..598bc97)
  - Task 2 every web copy a backend: done (42c59b4..01583ef)
  - Task 3 engine driver additions: done (a02dac2..b9f38ad)
  - Task 4 start one container copy: done (be6cf54..6e4fcf9)
  - Task 5 wait for health: done (2bf098e..068157e)
  - Task 6 rolling replacement: done (b32713d..06ff205)
  - Task 10 supervisor fixes: done (334b7aa)
  - Task 7 deploy container apps: done (db86594, cce8688, aad0492)
  - Task 8 rolling replacement for Procfile web copies: done (12aff7d)
  - Task 9 supervisor watches containers: done (4928fc2)
  - Task 11 migrate compose apps: done (4aef6ba)
  - Task 12 image watch: done (8144b8a)
  - Task 13 state API fields: done (e6e3734)
  - Task 14 dashboard rows and service actions: done (3c2a31b)
  - Task 15 CLI rows and logs: done (f67db2a)
  - Task 17 settings.toml and engine time limits: done (d32c08c..340ac0b)
  - Real Docker check fixes: done (b20c8a8, nginx include and host guard
    4294a66, merged 1fb66a4; filtered snippet check 1982cac)
  - Task 16 docs, changelog, roadmap: done
  - CI license gate: `webpki-root-certs` gets a one crate exception in
    `deny.toml` (owner, 2026-11-05). Every earlier CI run on the branch
    failed at this check, which also hid stale worker names in the Docker
    e2e cases, now fixed.
- Rootless engine (owner, 2026-11-04): spec
  `specs/2026-11-04-rootless-engine-design.md`, plan
  `plans/2026-11-04-rootless-engine.md`, refreshed 2026-11-05 against
  ffd7909: 19 tasks, Task 1 is a footprint gate with a 9,198,584 B
  baseline and a 10,247,160 B binary limit. Old Task 15 dropped (plugin
  deleted bb4aa56; `riku container` left to 3b, owner to confirm). Three
  open questions for the owner in the plan. Branch `feat/rootless-engine`
  off `feat/container-integration`. Tasks 1, 5 to 10 and R done. Tasks 2, 3
  and 4 (engine env, rootless probe, init detection) and the total cache cap,
  socket activation and duplicate key visitor were removed 2026-11-06.
- Owner decisions 2026-11-01: engine time limits in a new
  `~/.riku/settings.toml` (short calls 30 s, stop grace plus 30 s, pull and
  build 30 min); supervisor follows engine events and polls stats every
  30 s; restart counts reset on each deploy; nginx reload confirmation
  checked in a systemd container on the development machine.
- Docs tunnel fix (first small item): done.
- Phase 2 run ledger with every ruling, owner decision and about 60 deferred
  minor findings (several name phase 3 work):
  `.superpowers/sdd/2026-10-30-container-foundations/progress.md` (git
  ignored, this machine only). Read its "minor (deferred)", "Owner decided"
  and "Phase 3 note" lines while writing the plan.
- Duties from phase 2, each owned by a 3a task (the 3b ones moved to the
  phase 3b entry below):
- Compose migration duty from B4: a `Declared` project name (compose `name`,
  `COMPOSE_PROJECT_NAME`, the directory of a `COMPOSE_FILE` below the app
  root, or an app name that normalisation changes, such as `Shop` or `sh.op`)
  must be refused when it equals another riku app's name after both are
  normalised (`Shop`, `sh.op` and `shop` all compare as `shop`) or its
  existing volumes lack the matching compose project label, and only then may
  `ComposeProject::confirm_declared_name` be called.
- Before `run`, a container that already has the name but a different
  `riku.app` label is refused with a clear message (names can collide across
  apps: app `a-b` service `c` and app `a` service `b-c`).
- Compose migration removes compose's old containers by their compose project
  label before riku starts its own.
- The bind source symlink walk is repeated at container start.
- Each copy's host port is stored (as a label) so backends can be rebuilt
  after a supervisor restart.

### Phase 3b: build off the server through a registry (done, merged as #74)

- Owner cut 2026-11-06 after an EGM review: spec
  `specs/2026-11-06-phase-3b-design.md`. Images built off the server go
  through a registry and `image:` in compose; no upload path into riku. Delete
  riku-sync and `riku container`, add one docs page. Branch `feat/phase-3b`
  off main.
- The old 3b scope (`rk deploy --build`, platform query, image streaming,
  Procfile builds in a container, riku-sync wiring and its carried duties) is
  dropped.

### Developer experience: config files and the first run (next)

- Spec `specs/2026-11-09-developer-experience-design.md` (owner approved
  2026-11-09). Branch `feat/dev-experience`.
- Plan `plans/2026-11-09-config-model.md` (EGM reviewed, stages 5 to 9 plus
  one re-audit). Execution: native, one final review.
- Step 1 small fixes: done (Task 1 runtime bundles, Task 2 plugin docs).
- Project 1 config model: Tasks 3 (server settings), 4 (riku.toml schema),
  5 (app config loader), 6 (readers on the loader), 7 (riku settings),
  8 (doctor migration) and 9 (docs) done. Final gate and review next.
- Project 2 first run, split in two plans (Claude's call):
  - 2a setup: plan `plans/2026-11-10-first-run-setup.md`, EGM reviewed
    (stages 5 to 9 and one re-audit), branch `feat/first-run`, native
    execution. Tasks 1 (one workspace version), 2 (embedded runtime
    plugins, verified downloads), 3 (plugins install by runtime name),
    4 (router choice at init), 5 (name the missing runtime plugin), 6
    (deploy key and remote from rk server add) and 7 (first run guidance
    and docs) done. Merged in PR #89.
  - 2b URLs on first push: plan `plans/2026-11-10-url-on-first-push.md`,
    EGM reviewed (design and implementation readiness, both applied),
    branch `feat/url-first-push`, native execution. Owner confirmed
    refusing ports 80 and 443 in port mode. Task 1 (server routing
    settings, IP detection) and 2 (riku.toml mode and port, routing
    state, port rules) and 3 (fallback domain, certificate order,
    push URLs) done.

### After developer experience: small boards and binary size

- One task: build targets for Pi Zero / Zero W (armv6), Pi Zero 2 W and Pi
  3/4/5 (arm64, armv7), shipped as both glibc and static musl builds; a size
  profile trial (`opt-level = "z"`, `panic = "abort"`); sizes of riku and rk
  measured per target before and after. The owner sets a size budget from
  the measurements. (Moving ROUTER and CONTAINER_ENGINE into settings.toml
  is now part of the developer experience config model.)

### Phase 4: one rk managing many servers

- No design yet. The owner wants to be asked questions interactively first.
- Owner direction 2026-11-01: a fleet such as one controller board and ten
  host boards, driven from a PC. The controller is a hub (always on,
  aggregate dashboard, scheduling) that rk can bypass to reach a host
  directly when the controller is down.

## Known open problems, outside the plans

- nginx: `RIKU_NGINX_CACHE_PREFIXES` puts `proxy_cache_path` inside the
  `server` block; nginx allows it only at `http` level, so `nginx -t` fails
  for any app that sets it.
- nginx: `RIKU_NGINX_CATCH_ALL` adds a second `location /`, a duplicate
  wherever the template's default location or the uwsgi include is active.
  Read from the templates, not run against nginx.
- Certificates (`crates/riku-nginx/src/ssl.rs`): ACME issues per domain and
  stops at the first success, so other domains are uncovered; nginx serves
  the leaf `<app>.crt`, not `<app>.fullchain.crt`; `nginx_acme_firstrun`
  is registered but never selected. Needs a design.

- `riku install-plugins` failed to download inside a Docker test container.
  Cause: reqwest was built without a TLS backend, so every https request
  failed. Fixed with its `rustls-no-provider` feature plus rustls on ring
  (no aws-lc); `riku-http` installs ring before building any client.
- `generate_acme_nginx_config` (run by init) always fails to render:
  the template needs `RIKU_DOMAINS`, which it is never given. Its output
  file is now `acme-challenge.conf.disabled`, outside the `*.conf` include,
  so it stays off even once rendering works; turning it on needs a design.
- The agent forced command `command="riku agent --scope ..."` now parses,
  but sshd runs it with the client's request only in `SSH_ORIGINAL_COMMAND`,
  which `riku agent` never reads, so an agent behind that key sees only the
  help output. Reading it needs a design, and tokens from the client must
  never override the forced command's `--scope`.

---
title: "In-flight work on branch feat/caddy-router (RIKU_ env rename, Caddy router, compose port…"
author: Rakibul
date: 2026-10-26
id: l-f431b0f7
scope: repo
status: active
touches:
  - docs/superpowers/specs/2026-10-26-router-neutral-caddy-design.md
  - docs/superpowers/plans/2026-10-26-riku-env-rename.md
  - docs/superpowers/plans/2026-10-26-caddy-router.md
---

## Decided
Branch `feat/caddy-router` (local only, not pushed). Approved spec:
`docs/superpowers/specs/2026-10-26-router-neutral-caddy-design.md`.
Plan for Part 1 (env rename): `docs/superpowers/plans/2026-10-26-riku-env-rename.md`.

State (2026-10-26):
- Part 1 (env rename) DONE, committed on the branch: 242e31b (legacy key table in `riku-util/src/legacy_env.rs`, deploy hint, `riku doctor` app env check) and 52efb84 (mechanical rename). Next: Part 2 (Caddy built in) needs its own plan.
- Part 2 plan `docs/superpowers/plans/2026-10-26-caddy-router.md` (6 tasks) approved and in execution. All 6 Caddy plan tasks committed (last: 8e1ecca docs). Verified in Docker: caddy adapt + live redirect behavior, mkdocs --strict, root init symlink attacks refused, clean root init idempotent. Root init reworked in cccca6d: stages root-owned binary in a temp dir, runs `init --user-steps` as deploy (setsid, no tty, env allowlist), root does host setup only; verified as real root in Debian container. Relayed output stripped of control chars (5379bc4). `riku init --ssh-key <file>` added (093db93), verified as root. Interactive key path fixed (90571a7). Branch pushed, PR #71 open against main (2026-10-26). Docker image caddy:2, python:3.12-slim, rust:1-bookworm, debian:bookworm-slim pulled for checks. Root init now writes ROUTER into the deploy user tree (owner choice) and requires the deploy user to exist; Task 6 docs must say so. Deploy tests (e2e, stress) write ROUTER=caddy so they need no nginx binary;
…

## Watch out
On resume, verify the branch and files still match this note, then ask for plan approval. Related: no-python-file-edits, riku-single-user.

_From Claude Code's memory of this repo, last updated Oct 26._

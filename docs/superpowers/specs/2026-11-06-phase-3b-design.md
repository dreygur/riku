# Phase 3b: build off the server through a registry

Owner approved 2026-11-06. Replaces the 3b parts of
`2026-10-30-container-services-design.md` (lines 163-203).

## Goal

A small, secure, self hosted Heroku replacement: a small core extended by
plugins. A user on a small board must be able to run container apps without a
build toolchain on the server. `git push` stays the way code reaches the
server.

## Decision

Images built off the server travel through a registry, not a new upload path
into riku. The user builds and pushes on their laptop (or in CI) and names the
image in compose with `image:`. Riku already pulls `image:` services and logs
in to GHCR from `RIKU_GHCR_USERNAME` and `RIKU_GHCR_TOKEN`
(crates/riku-deploy/src/containers/images.rs:24-58). A host without an outside
registry can run a registry container itself.

An EGM review (2026-11-06) of a stdin `riku image-load` design found that a
loaded archive applies its own image names, so one upload could replace
another app's image; making it safe meant parsing archives in Rust, a skip
rule for the server build and core changes on the laptop side. The registry
path needs none of that.

## Scope

1. Delete the riku-sync crate. Nothing uses it; it brings tar and zstd. Git
   history keeps it.
2. Delete `riku container` (build, export, remote deploy, check remote), its
   supervisor control action and every reference. It wrote an archive to the
   working directory and ran `ssh host '<engine> load'`, which the forced
   command parses as a riku command, so it never worked against a riku server.
   Breaking change: one CHANGELOG entry.
3. One docs page: building on a laptop or in CI for the server's platform
   (`docker buildx build --platform linux/arm64 --push`), naming the image in
   compose, GHCR login settings, and running a registry on the server.

Not in scope: an image upload command, `rk deploy --build`, a platform query,
Procfile builds in a container (a runtime plugin can do it with
`RIKU_RUNTIME`), and the sync duties carried from phase 2, which go with the
crate.

## Security model

No new trust boundary: riku gains no command and no input. Pulls run as
today, with the engine and credentials riku already uses.

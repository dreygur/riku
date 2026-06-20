# GHCR Image Deploy Example

For apps where **CI builds the image, not riku**: GitHub Actions builds your
app, pushes it to `ghcr.io`, then a `git push` (the same trigger every other
riku deploy uses) tells riku to pull and run the new image. No SSH-pushing
your actual app source to riku at all — riku never builds anything here, it
just pulls whatever tag CI pushed.

## How the pieces fit together

```
GitHub Actions (in your app's repo)
  -> docker build + push to ghcr.io/you/app:latest
  -> git push (empty commit) to this minimal "trigger" repo on the riku host
       -> post-receive hook -> riku deploy
            -> ghcr plugin: docker pull ghcr.io/you/app:latest
            -> ghcr plugin: docker run --rm -p $PORT:$PORT riku-<app>
```

The riku-side repo (this directory) contains **no application code** — just
a `Procfile` and `ENV` telling riku which image to pull. Your actual app
source lives in its own repo with its own Dockerfile; riku only ever sees
the image it builds, never the source.

## Setup

**1. On the riku host**, create the app and point it at your image:

```bash
ssh deploy@your-server riku apps:create myapp   # or push once to auto-create
riku config set myapp RUNTIME=ghcr
riku config set myapp GHCR_IMAGE=ghcr.io/you/app:latest
```

**Resource limit caveat (read this before testing):** riku caps every plugin
`build` step's virtual address space (`RLIMIT_AS`, default 512 MB — see
[resource-limits.md](../../docs/docs/resource-limits.md)) to stop a runaway
build from exhausting host memory. `docker`/`podman` (and `go`) are Go
binaries, and the Go runtime reserves a large virtual address space for its
heap arena at *startup* regardless of actual usage — no finite `RLIMIT_AS`
compatible with normal host RAM avoids this (confirmed failing even at
32 GB). Set:

```bash
export RIKU_MAX_MEMORY_MB=unlimited
```

**in the environment of the process that runs `riku git-hook`/`riku
deploy`** — i.e. wherever `git push`'s post-receive hook actually executes
(an SSH session for the `deploy` user), not the `riku supervisor` systemd
unit's `Environment=` (that only governs already-running *workers*, a
separate process tree). The most portable place is system-wide in
`/etc/environment` on the riku host; confirm it took with `RUST_LOG=info`
and look for `Resource limit: max_memory = unlimited` in the deploy log.

If the image is private, log the riku host in once:

```bash
docker login ghcr.io -u you -p <personal-access-token>
```

**2. Push this directory's `Procfile`/`ENV`** to riku once, to create the
worker:

```bash
cd examples/ghcr-image
git init && git add . && git commit -m "init"
git remote add riku deploy@your-server:myapp
git push riku main
```

**3. In your app's actual repo**, add [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml)
(adjust paths/branch as needed) and set two repo secrets:

| Secret | Value |
|---|---|
| `RIKU_DEPLOY_KEY` | private half of an SSH key whose public half is in the riku host's `~/.ssh/authorized_keys` for the `deploy` user (see [installation](../../docs/docs/installation.md)) |
| `RIKU_REMOTE` | `deploy@your-server:myapp` |

Every push to `main` now: builds your image, pushes it to GHCR, then pushes
an empty commit to riku to redeploy — pulling the tag that was *just*
pushed, not a stale cached build.

## Why a `git push`, not a webhook/curl?

Riku does have a control-plane HTTP API (`POST /control/apps/:app/deploy`),
but it's gated by a bearer token meant for the dashboard's own server-side
proxy, not designed to be exposed to the public internet from a CI runner.
Reusing the existing SSH-authenticated `git push` trigger needs no new
network exposure or auth surface on the riku host — same trust boundary as
every other riku deploy. If your CI runner already lives inside the same
private network as riku (e.g. a self-hosted runner), curling the control
API directly is a valid alternative; just don't expose that port publicly
without your own auth in front of it.

## See Also

- [Plugin System Documentation — the `ghcr` plugin](../../docs/docs/plugins.md#the-ghcr-plugin-ci-built-images)
- [Server-Side Hooks](../server-hooks/README.md) — e.g. a `riku-post-deploy`
  Slack notification once the new image is running

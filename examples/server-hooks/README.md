# Riku Server-Side Hook Plugins

Server-side hook plugins let you run custom scripts at defined points in the
deploy pipeline. Drop an executable into `~/.riku/plugins/` named after the
hook and Riku will invoke it automatically on every deploy.

## Hook Execution Order

```
git push received
  → code checked out
  → env vars loaded
  → [pre-deploy]   ← validate env, abort if required vars are missing
  → runtime detected
  → [pre-build]    ← install extra deps, patch sources
  → runtime build step
  → [post-build]   ← run tests, database migrations
  → worker configs written
  → [post-deploy]  ← send notifications, warm caches
```

## The Four Hooks

| Hook          | Plugin file            | When it fires                        |
|---------------|------------------------|--------------------------------------|
| `pre-deploy`  | `riku-pre-deploy`      | After env load, before build         |
| `pre-build`   | `riku-pre-build`       | After runtime detection, before build|
| `post-build`  | `riku-post-build`      | After build, before workers start    |
| `post-deploy` | `riku-post-deploy`     | After workers are started            |

Exit non-zero from any hook to abort the deploy and print an error to the
deploy log. `post-deploy` hooks cannot abort the deploy (workers are already
running), but a non-zero exit is still recorded in the logs.

## Environment Variables

Every hook receives the following variables:

| Variable        | Description                                      | Example                          |
|-----------------|--------------------------------------------------|----------------------------------|
| `RIKU_APP`      | Application name                                 | `myapp`                          |
| `RIKU_HOOK`     | Hook name                                        | `pre-deploy`                     |
| `RIKU_APP_PATH` | Path to the checked-out source code              | `/home/deploy/.riku/apps/myapp`  |
| `RIKU_ENV_PATH` | Path to the app's env directory                  | `/home/deploy/.riku/envs/myapp`  |
| `RIKU_ROOT`     | Riku root directory                              | `/home/deploy/.riku`             |
| `RIKU_RUNTIME`  | Detected runtime (empty in `pre-deploy`)         | `Python`                         |

All variables from the app's `ENV` file are also passed through, so
`DATABASE_URL`, `SECRET_KEY`, etc. are available directly.

## Installation

```bash
# Copy a hook plugin into place
cp examples/server-hooks/riku-pre-deploy ~/.riku/plugins/riku-pre-deploy
chmod +x ~/.riku/plugins/riku-pre-deploy
```

That's it — the plugin fires automatically on the next `git push`.

## Example Plugins in This Directory

| File               | What it does                                      |
|--------------------|---------------------------------------------------|
| `riku-pre-deploy`  | Validates required env vars before deploy starts  |
| `riku-post-build`  | Runs Django database migrations after build       |
| `riku-post-deploy` | Posts a Slack notification after deploy           |

## See Also

- [Plugin System Documentation](../../docs/docs/plugins.md)
- [Client-Side Plugins](../client-plugins/README.md)
- [Environment Variables](../../docs/docs/env.md)

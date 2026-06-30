# Rollback

Every deploy creates a **release**. If a deploy goes wrong, roll back to a
previous release without re-pushing or restoring from a backup.

## Roll back to the previous release

```bash
riku rollback myapp
```

With no options, Riku rolls the app back to the release immediately before the
current one.

## Roll back to a specific release

```bash
riku rollback myapp --to <sha>
```

`--to` takes a commit SHA from the release history.

## List the release history

```bash
riku rollback myapp --list
```

This prints the recorded releases (instead of rolling back), so you can pick the
SHA you want to return to.

## Options

| Option | Description |
|--------|-------------|
| `--to <TO>` | Roll back to a specific commit SHA (default: the previous release) |
| `--list` | List the release history instead of rolling back |

## Examples

```bash
# See what you can roll back to
riku rollback myapp --list

# Go back one release
riku rollback myapp

# Pin to a known-good commit
riku rollback myapp --to a1b2c3d
```

## Rollback vs. restore

| | [Rollback](rollback.md) | [Restore](backup-restore.md) |
|---|-------------------------|------------------------------|
| Source | A previous **release** already on the server | An external `.tar.gz` backup |
| Scope | App code/version | Source + env + volumes + repo |
| Use when | A bad deploy needs reverting fast | Rebuilding or migrating an app |

!!! tip "Zero-downtime"
    Riku performs graceful restarts, so rolling back swaps to the previous
    release without dropping in-flight requests. See
    [Supervisor](supervisor.md) for how process restarts are managed.

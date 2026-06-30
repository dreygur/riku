# Backup & Restore

Riku can snapshot an entire app to a single `.tar.gz` and restore it later — on
the same server or a different one. A backup bundles everything that makes the
app reproducible:

- **Source** — the checked-out application code
- **Env** — the app's environment variables
- **Volumes** — persistent data volumes
- **Repo** — the bare git repository

## Back up an app

```bash
# Writes ./<app>-backup-<timestamp>.tar.gz
riku backup myapp

# Choose the output path
riku backup myapp --out /backups/myapp.tar.gz
```

| Option | Description |
|--------|-------------|
| `--out <OUT>` | Output path. Defaults to `./<app>-backup-<timestamp>.tar.gz` |

## Restore an app

```bash
riku restore myapp ./myapp-backup-20260624.tar.gz
```

`restore` takes the target app name and the backup file, recreating the app's
source, env, volumes, and repository from the archive.

```text
riku restore <APP> <FILE>
```

## Backing up addon data

App backups capture the app's own files and env, including any addon connection
strings. The **data inside a managed addon** (e.g. the actual rows in a Postgres
database) is backed up separately, through the addon:

```bash
riku addon backup db1
```

See [Addons](addons.md) for the full addon lifecycle.

## Recommended routine

!!! tip "Automate it"
    Schedule `riku backup` from cron (or a Procfile `cron:` entry) and copy the
    resulting archive off-box. Combine with `riku addon backup` for each
    attached datastore.

```bash
# Example: nightly app + database backup, copied to remote storage
0 2 * * * riku backup myapp --out /backups/myapp-$(date +\%F).tar.gz
0 2 * * * riku addon backup db1
```

## Disaster recovery

To move an app to a fresh server:

1. `riku backup myapp --out myapp.tar.gz` on the old server.
2. Copy the archive to the new server.
3. `riku init` (if not already initialized) on the new server.
4. `riku restore myapp myapp.tar.gz`.
5. Re-provision and bind any addons, then `riku restart myapp`.

See also [Rollback](rollback.md) for reverting to a previous release without a
full restore.

# Plugin Gallery

Ready-made [plugin bundles](plugin-bundles.md) that ship in the Riku repo under
[`examples/plugins/`](https://github.com/dreygur/riku/tree/main/examples/plugins).
Each is a working reference you can install as-is or copy as a starting point.

| Plugin | Type | What it does | Capabilities |
| ------ | ---- | ------------ | ------------ |
| [`sqlite-volume`](#sqlite-volume) | addon | Persistent SQLite database file on a managed volume — no external services | `writes` |
| [`postgres`](#postgres) | addon | Managed PostgreSQL database + role on the host | `network`, `writes` |
| [`webhook-notify`](#webhook-notify) | notifier | POST lifecycle events to a webhook | `network` |

Install any of them from a local checkout:

```bash
riku plugins install ./examples/plugins/<name>
```

---

## sqlite-volume

A zero-dependency addon giving an app a persistent SQLite file — true to Riku's
single-box identity.

```bash
riku plugins install ./examples/plugins/sqlite-volume
riku addon create sqlite-volume mydb
riku addon bind mydb myapp
```

On bind, the app's env gains `DATABASE_URL=sqlite:///…` and `SQLITE_PATH`. The
database lives under `~/.riku/data/addons/sqlite-volume/<instance>/` and survives
redeploys. `backup` copies the file to a timestamped artifact.

## postgres

Provisions a database and login role on a PostgreSQL server reachable via the
standard `PG*` environment variables. Requires `psql` (and `pg_dump` for backup)
on the host.

```bash
riku plugins install ./examples/plugins/postgres
riku addon create postgres db1
riku addon bind db1 myapp           # injects DATABASE_URL
```

`deprovision` drops the database and role; the kernel refuses it while any app is
still bound.

## webhook-notify

A notifier (event subscriber) that POSTs each subscribed lifecycle event to a
webhook.

```bash
riku plugins install ./examples/plugins/webhook-notify
riku config set <app> WEBHOOK_URL=https://hooks.example.com/...
```

It subscribes to `deploy.requested`, `build.finished`, and `deploy.finished`
(observe mode — failures are logged, never block a deploy).

---

## Contributing a plugin

1. Scaffold a bundle (`riku-plugin.toml` + `bin/`), following
   [Plugin Bundles](plugin-bundles.md).
2. Sign it (`riku plugins keygen` → `riku plugins sign`) so operators can verify
   provenance.
3. Publish it as a git repo, and (optionally) list it in a marketplace's
   `marketplace.toml` so others can `riku plugins search` / `add` it.

The full contract — verbs, I/O schema, the event catalog, and the trust model —
is in [`PLUGIN_PROTOCOL.md`](https://github.com/dreygur/riku/blob/main/PLUGIN_PROTOCOL.md).

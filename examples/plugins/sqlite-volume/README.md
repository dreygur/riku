# sqlite-volume

A Riku **addon** (Plugin Protocol v1) that gives an app a persistent SQLite
database file on a managed volume — no external database server, true to Riku's
single-box identity.

## Install (local/dev)

```sh
cp -r examples/plugins/sqlite-volume ~/.riku/plugins/
chmod +x ~/.riku/plugins/sqlite-volume/bin/addon

riku addon create sqlite-volume mydb     # provision an instance
riku addon bind mydb myapp               # inject DATABASE_URL into myapp
```

On bind, the app's env gains:

- `DATABASE_URL=sqlite:///<data>/mydb.db`
- `SQLITE_PATH=<data>/mydb.db`

The database file lives under `~/.riku/data/addons/sqlite-volume/<instance>/`
and survives redeploys.

## Verbs

| Verb          | Behavior                                              |
| ------------- | ----------------------------------------------------- |
| `provision`   | Create the volume + an empty `.db` file               |
| `bind`        | Return `DATABASE_URL` / `SQLITE_PATH` for the app     |
| `unbind`      | No-op (the kernel removes the injected env)           |
| `deprovision` | No-op (the kernel deletes the data dir)               |
| `backup`      | Copy the `.db` to a timestamped artifact              |

# postgres

A Riku **addon** (Plugin Protocol v1) that provisions a managed PostgreSQL
database and login role per instance. This is the roadmap's named keystone
addon.

## Requirements

- PostgreSQL running on (or reachable from) the host.
- `psql` available to the deploy user, and `pg_dump` for `backup`.
- The addon connects as an admin via the standard `PG*` env vars
  (`PGHOST`, `PGPORT`, `PGUSER`, …); by default the local socket as a
  superuser. Set these in the deploy user's environment as needed.

## Install (local/dev)

```sh
cp -r examples/plugins/postgres ~/.riku/plugins/
chmod +x ~/.riku/plugins/postgres/bin/addon

riku addon create postgres db1     # CREATE ROLE + CREATE DATABASE
riku addon bind db1 myapp          # inject DATABASE_URL into myapp
```

On bind, the app's env gains `DATABASE_URL=postgres://…`. The generated role
password is stored (mode 600) under
`~/.riku/data/addons/postgres/<instance>/credentials`.

## Verbs

| Verb          | Behavior                                          |
| ------------- | ------------------------------------------------- |
| `provision`   | Create role + database, generate a password       |
| `bind`        | Return the `DATABASE_URL` for the app             |
| `unbind`      | No-op (the kernel removes the injected env)        |
| `deprovision` | `DROP DATABASE` + `DROP ROLE`                      |
| `backup`      | `pg_dump` to a timestamped artifact               |

> The database is **not** dropped until `riku addon destroy <instance>`, which
> the kernel refuses while any app is still bound.

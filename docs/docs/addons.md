# Addons

Addons are **managed resources** — databases, caches, and other stateful
services — that Riku provisions and binds to your apps. Each addon is an
[addon-type plugin](plugin-bundles.md); the core stays a single binary while the
ecosystem provides the datastores.

Officially bundled addons (in the [marketplace](marketplace.md)):

| Addon | Description | Host requirement |
|-------|-------------|------------------|
| `postgres` | Managed PostgreSQL database and login role | PostgreSQL + `psql` (`pg_dump` for backup) |
| `redis` | Managed Redis instance via per-instance ACL users | Redis server |
| `sqlite-volume` | Persistent SQLite file on a managed volume | None |

## Concepts

- **Plugin** — the addon implementation (e.g. `postgres`), installed once.
- **Instance** — a provisioned resource (e.g. a database named `db1`).
- **Bind** — attaching an instance to an app injects its connection env
  (e.g. `DATABASE_URL`) into that app.

An instance must be **unbound** before it can be destroyed.

## Install an addon plugin

```bash
# From the official marketplace, by name
riku plugins add postgres

# Or from a local path / git URL
riku plugins install ./examples/plugins/postgres
```

See [Marketplace](marketplace.md) for registering sources and verifying
signatures.

## Provision and bind

```bash
# Create an instance named db1 from the postgres addon
riku addon create postgres db1

# Bind it to an app — injects DATABASE_URL into myapp's environment
riku addon bind db1 myapp

# Restart so the app picks up the new env
riku restart myapp
```

After binding, the app's environment contains the connection string the addon
emits. For `postgres` that is:

```
DATABASE_URL=postgres://riku_db1:<password>@<host>:5432/riku_db1
```

The instance name is sanitized into a safe SQL identifier, the password is
randomly generated, and the credentials file is written `chmod 600`.

## Command reference

```text
riku addon list                      # List provisioned instances
riku addon create <PLUGIN> <NAME>    # Provision a new instance
riku addon bind <INSTANCE> <APP>     # Bind an instance to an app
riku addon unbind <INSTANCE> <APP>   # Remove the instance's env from an app
riku addon destroy <INSTANCE>        # Destroy an instance (unbind first)
riku addon backup <INSTANCE>         # Back up an instance
```

### Examples

```bash
riku addon list
riku addon create postgres db1
riku addon bind db1 myapp
```

## Lifecycle

```text
create ──▶ bind ──▶  (app uses it)  ──▶ unbind ──▶ destroy
```

Tearing an instance down:

```bash
riku addon unbind db1 myapp
riku addon destroy db1
```

## How addons work under the hood

An addon plugin is an executable that implements the Plugin Protocol verbs over
JSON on stdin/stdout: `provision`, `bind`, `unbind`, `deprovision`, and
`backup`. Riku passes context via environment variables:

- `RIKU_ADDON_INSTANCE` — the instance name
- `RIKU_ADDON_DATA_PATH` — a per-instance data directory for credentials/state

To author your own addon, scaffold a bundle and implement those verbs:

```bash
riku plugins scaffold
```

See the [Plugin Protocol](https://github.com/dreygur/riku/blob/main/PLUGIN_PROTOCOL.md)
for the full contract.

!!! warning "Host prerequisites"
    Managed addons drive software already installed on the host. The `postgres`
    addon needs a reachable PostgreSQL server and `psql` (it connects via the
    standard `PG*` environment variables, defaulting to the local socket as an
    admin user). Install and secure the underlying service before provisioning.

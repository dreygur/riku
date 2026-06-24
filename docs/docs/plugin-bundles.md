# Plugin Bundles (Protocol v1)

Riku's modern extension model is the **plugin bundle** — a directory with a
`riku-plugin.toml` manifest and one or more executables. Bundles are installed,
versioned, checksum-verified, and (optionally) signature-verified through the
`riku plugins` commands.

> This is distinct from the legacy single-file plugins in
> [Plugin System](plugins.md). Bundles are the path forward; the full contract
> is specified in [`PLUGIN_PROTOCOL.md`](https://github.com/dreygur/riku/blob/main/PLUGIN_PROTOCOL.md).

## Bundle layout

```
my-plugin/
  riku-plugin.toml      # manifest
  bin/                  # executable(s) implementing the type's verbs
  README.md
```

## Manifest

```toml
name        = "postgres"
version     = "1.2.0"
type        = "addon"            # runtime | addon | notifier | hook | router
api         = 1                  # RIKU_PLUGIN_API this targets
entry       = "bin/riku-postgres"
description = "Managed PostgreSQL addon"
author      = "you@example.com"
checksum    = "sha256:…"         # optional; verified on install
signature   = "…"               # optional; Ed25519 over the entry, verified
                                 # against the operator's trusted keys

[capabilities]                   # declared, shown on install
network     = true
writes      = ["app_dir", "data_dir"]
privileged  = false

[events]                         # for event subscribers (notifier/hook)
subscribe   = ["deploy.finished", "deploy.failed"]
mode        = "observe"          # observe | gate
```

The kernel sets `RIKU_PLUGIN_API`, `RIKU_ROOT`, and (when app-scoped) `RIKU_APP`,
`RIKU_APP_PATH`, `RIKU_ENV_PATH` in the plugin's environment on every call.

## Plugin types (seams)

| Type | Verbs | Status |
| ---- | ----- | ------ |
| **runtime** | `detect` / `build` / `env` / `start` | shipped (buildpacks) |
| **addon** | `provision` / `bind` / `unbind` / `deprovision` / `backup` | shipped |
| **notifier / hook** | `on_event` (subscribes to lifecycle events) | shipped |
| **router** | `configure` / `reload` | planned |

### Addons

An **addon** is a managed resource (database, cache, …). Each install can be
provisioned into named **instances**, each bound to apps; binding injects env
(e.g. `DATABASE_URL`) into the app:

```bash
riku addon create postgres db1     # provision an instance
riku addon bind db1 myapp          # inject its env into myapp
riku addon list
riku addon unbind db1 myapp
riku addon destroy db1             # refused while bound
```

### Event subscribers (notifiers)

A bundle with an `[events]` block is invoked with `on_event` and the event JSON
on stdin for each subscribed lifecycle event (`deploy.requested`,
`build.finished`, `deploy.finished`, …). `observe` mode is fire-and-forget;
`gate` mode (veto on pre-phase events) requires elevated trust.

## Installing & managing

```bash
riku plugins install ./my-plugin        # from a local path
riku plugins install github:owner/repo  # from a git repo
riku plugins list                       # name, version, type, verified
riku plugins remove my-plugin
riku plugins doctor                     # validate api + integrity (tamper check)
```

## Marketplaces

A marketplace is a git repo whose `marketplace.toml` indexes plugins. It is
**git-native — no central server**:

```bash
riku plugins marketplace add github:dreygur/riku-marketplace
riku plugins marketplace list
riku plugins search postgres            # reads the index only
riku plugins add postgres               # resolve via marketplace + install
riku plugins add postgres@official      # disambiguate by marketplace
```

Registering a marketplace lets it publish code that runs on your server, so it
is an explicit trust decision (Riku warns on `add`).

## Trust & security

Riku plugins run on the server as the deploy user, so installs are gated:

- **Checksum** — a manifest-pinned `sha256` is rejected on mismatch; the
  computed digest is recorded in `riku-plugins.lock` regardless, so
  `riku plugins doctor` can later detect tampering.
- **Signatures** — an author signs the entry with an Ed25519 key; the operator
  trusts publisher keys. A signed bundle installs only if a trusted key verifies
  it, else it is **rejected**.

  ```bash
  # Author
  riku plugins keygen --out signing.key
  riku plugins sign ./my-plugin --key signing.key

  # Operator
  riku plugins trust add acme <public-key-hex>
  riku plugins install ./my-plugin     # accepted only if a trusted key verifies
  ```

- **Capabilities** — `network` / `writes` / `privileged` are declared in the
  manifest and shown on install (informed consent).
- **Lockfile** — `riku-plugins.lock` pins each install's name, source, version,
  checksum, and verifying key. No silent auto-update of executable code.

See the [Plugin Gallery](plugin-gallery.md) for ready-made examples.

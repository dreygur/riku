# Plugin Bundles (Protocol v1)

Riku's modern extension model is the **plugin bundle**, a directory with a
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
priority    = 0                  # delivery order among subscribers of the
                                  # same event, lower first (default 0)
emit        = false              # may this plugin fire its own plugin.custom.*
                                  # events via `riku plugin-emit`?

[lifecycle]                      # optional, any plugin type: install/remove hooks
install     = true               # `riku plugins install` calls on_install
uninstall   = true               # `riku plugins remove` calls on_uninstall

[ui]                              # optional, any plugin type: dashboard panel
nav_label   = "My Plugin"        # Next.js dashboard only, not the embedded one
```

The kernel sets `RIKU_PLUGIN_API`, `RIKU_ROOT`, `RIKU_PLUGIN_NAME` (the
manifest's own `name`), and `RIKU_PLUGIN_DATA_PATH` (a scratch directory the
plugin owns, `data_root/plugin-data/<name>/`, created lazily) in the
plugin's environment on every call, plus (when app-scoped) `RIKU_APP`,
`RIKU_APP_PATH`, `RIKU_ENV_PATH`.

### Install/uninstall lifecycle

A plugin declaring `[lifecycle]` gets `on_install` called right after its
files are copied into `~/.riku/plugins/`, and/or `on_uninstall` right before
they're removed: both best-effort (a failing hook is logged, never blocks
the install or removal it's attached to). Not declaring `[lifecycle]` at all
(every plugin shipped before this existed, including `riku-notify`) means
neither verb is ever invoked.

## Plugin types (seams)

| Type | Verbs | Status |
| ---- | ----- | ------ |
| **runtime** | `detect` / `build` / `env` / `start` | shipped (buildpacks) |
| **addon** | `provision` / `bind` / `unbind` / `deprovision` / `backup` | shipped |
| **notifier / hook** | `on_event` (subscribes to lifecycle events) | shipped |
| **router** | `configure` / `reload` | shipped: singleton, full-replace (`RIKU_ROUTER=<name>`) |
| **filter** (any type) | `on_filter` (`[filters]` block, any plugin type) | shipped, chained, augment-only |
| **UI panel** (any type) | `ui_panel` (`[ui]` block, any plugin type) | shipped, Next.js dashboard only, read-only |

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
`build.finished`, `deploy.finished`, `app.restarted`, `app.failed`, …).
`observe` mode is fire-and-forget; `gate` mode (veto on pre-phase events)
requires elevated trust.

**Shipped example: `riku-notify`.** Bundled at `plugins/riku-notify/` (a
`riku-plugin.toml` + a POSIX shell `bin/on-event`), it subscribes to both
`app.restarted` (a crash the supervisor recovered from) and `app.failed` (a
crash that exceeded `max_restarts`: riku has given up on the instance
entirely, the more urgent of the two), and posts an incident report, the
crashed instance, exit code, and restart count, to whichever channels are
configured, each independent:

| Env var | Channel |
| --- | --- |
| `RIKU_NOTIFY_WEBHOOK_URL` | Generic JSON POST |
| `RIKU_NOTIFY_DISCORD_WEBHOOK_URL` | Discord incoming webhook |
| `RIKU_NOTIFY_SLACK_WEBHOOK_URL` | Slack incoming webhook |
| `RIKU_NOTIFY_TELEGRAM_BOT_TOKEN` + `RIKU_NOTIFY_TELEGRAM_CHAT_ID` | Telegram bot message |

Unlike third-party bundles, it's first-party (shipped in the same repo as the
`riku` binary), so it installs through the simpler bundled-plugin downloader
rather than the checksum/signature path below:

```bash
riku install-plugins --plugins riku-notify
```

### Filters

A bundle with a `[filters]` block is invoked with `on_filter` and
`{"filter": "<name>", "data": <value>}` on stdin, expected to return
`{"data": <transformed value>}`. Unlike events, filters transform a value
and hand it back rather than firing-and-forgetting. Multiple plugins on the
same filter name run as a **chain**, ordered by `filters.priority` (lower
first), each seeing the previous one's output.

**Must degrade safely**: a broken, timed-out, or malformed-output filter is
skipped (logged) and the value passes through unchanged, a filter can only
become a no-op, never break the thing calling it.

**Shipped example: `nginx.include_content`.** The nginx config generator
already supported a raw file-based passthrough (`NGINX_INCLUDE_FILE`); that
content is now the *seed* value run through this filter before being
inlined into the generated `.conf`. This is the "augment the default config"
alternative to writing a full router plugin (below), any number of
installed plugins can each contribute a snippet, rather than one plugin
replacing routing entirely.

```toml
[filters]
subscribe = ["nginx.include_content"]
priority  = 0
```

### Custom events

Riku's own lifecycle events (`app.restarted`, `deploy.finished`, …) are
kernel-emitted only: plugins can't fire those, and that's deliberate: the
event stream is only trustworthy if plugins can't forge it. Instead, a
plugin that declares `[events] emit = true` can fire its **own** events for
other plugins to subscribe to:

```bash
# from inside the emitting plugin's own script
riku plugin-emit plugin.custom.backup-done --data '{"ok":true}'
```

- The event name **must** start with `plugin.custom.`: anything else is
  rejected outright (a plugin can never emit something that looks like a
  kernel event, e.g. its own fake `app.restarted`).
- The kernel identifies the caller itself (via `RIKU_PLUGIN_NAME`, always
  set) and checks *that* plugin's manifest declared `emit = true`, a
  plugin can't claim to be a different one.
- Subscribers receive the same envelope shape as any other event, plus
  `source_plugin: "<name>"`, kernel-set, so you can always tell a
  kernel-truth event (`source_plugin: null`) from a plugin-claimed one.

### UI panels

A plugin declaring `[ui] nav_label = "..."` gets a nav entry and a page in
the **Next.js browser dashboard** (`dashboard/`: not the embedded
single-binary one, which has no plugin-panel seam). The dashboard dispatches
`ui_panel` (no meaningful stdin) and expects structured JSON back:

```json
{"sections": [{"title": "Status", "fields": [{"label": "Queue depth", "value": "12"}]}]}
```

**Structured data only, never HTML/JS.** This is the deliberate scope-limiter
that closes off injection risk: a plugin supplies labels and values, the
dashboard renders them with its own components, nothing a plugin returns is
ever interpreted as markup. There's no verb for a plugin-defined *action*
(a button calling back into the plugin) yet: this is read-only display.

Same degrade-safely contract as filters: a broken, timed-out, or
malformed-output panel logs a warning and renders as empty, never breaks the
dashboard.

```toml
[ui]
nav_label = "My Plugin"
```

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
**git-native: no central server**:

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

- **Checksum**: a manifest-pinned `sha256` is rejected on mismatch; the
  computed digest is recorded in `riku-plugins.lock` regardless, so
  `riku plugins doctor` can later detect tampering.
- **Signatures**: an author signs the entry with an Ed25519 key; the operator
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

- **Capabilities**: `network` / `writes` / `privileged` are declared in the
  manifest and shown on install (informed consent).
- **Lockfile**: `riku-plugins.lock` pins each install's name, source, version,
  checksum, and verifying key. No silent auto-update of executable code.

See the [Plugin Gallery](plugin-gallery.md) for ready-made examples.

# plugin_author

The `plugins scaffold`, `keygen` and `sign` commands, which a plugin author
runs on their own machine. `riku` and `rk` both call them from here.

## Interface
- `cmd_plugins_keygen` [keys.rs:cmd_plugins_keygen] (fn)
- `cmd_plugins_sign` [keys.rs:cmd_plugins_sign] (fn)
- `Program` [mod.rs:Program] (enum): which binary (`riku` or `rk`) is running, so printed follow-up commands name it
- `cmd_plugins_scaffold` [scaffold.rs:cmd_plugins_scaffold] (fn, re-exported from mod.rs)
- `SeamType` [skeleton.rs:SeamType] (enum): the bundle type to scaffold, with `parse`, `manifest` and `entry_script`

## Invariants
- [mod.rs] The bundle name is validated before it becomes a path
- [scaffold.rs] Scaffolding refuses to write over an existing path
- [keys.rs] The secret key is written with mode 0600 from creation and never printed; signing covers the entry file only, as described in `riku-bundle`

## Side Effects
- [scaffold.rs] Creates the bundle directory: `riku-plugin.toml`, `bin/<entry>` (mode 0755) and `README.md`
- [keys.rs] Writes the secret key file and the signature line into `riku-plugin.toml`

## Verification
- Run: `cargo test -p riku-remote`

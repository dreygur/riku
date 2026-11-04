# riku-plugins (src)

## Overview
Component package located at `crates/riku-plugins/src`.

## Interface
- `find_bundles` [bundles.rs:find_bundles] (fn)
- `find_addon` [bundles.rs:find_addon] (fn)
- `find_router` [bundles.rs:find_router] (fn)
- `bundles.rs` [bundles.rs] (and 1 additional symbols)
- `validate_plugin_name` [discovery.rs:validate_plugin_name] (fn)
- `list_plugins` [discovery.rs:list_plugins] (fn)
- `plugin_exists` [discovery.rs:plugin_exists] (fn)
- `plugin_timeout` [executor.rs:plugin_timeout] (fn)
- `tee_output` [executor.rs:tee_output] (fn)
- `capture_stdout` [executor.rs:capture_stdout] (fn)
- `executor.rs` [executor.rs] (and 5 additional symbols)
- `PluginHook` [hooks.rs:PluginHook] (enum)
- `plugin_name` [hooks.rs:plugin_name] (fn)
- `hook_name` [hooks.rs:hook_name] (fn)
- `hooks.rs` [hooks.rs] (and 2 additional symbols)
- `checksum_of` [install.rs:checksum_of] (fn)
- `PluginInstaller` [install.rs:PluginInstaller] (struct)
- `new` [install.rs:new] (fn)
- `install.rs` [install.rs] (and 8 additional symbols)
- `addon` [lib.rs:addon] (module)
- `bundles` [lib.rs:bundles] (module)
- `discovery` [lib.rs:discovery] (module)
- `lib.rs` [lib.rs] (and 21 additional symbols)
- `LockEntry` [lockfile.rs:LockEntry] (struct)
- `Lockfile` [lockfile.rs:Lockfile] (struct)
- `new` [lockfile.rs:new] (fn)
- `lockfile.rs` [lockfile.rs] (and 3 additional symbols)
- `PluginManager` [manager.rs:PluginManager] (struct)
- `new` [manager.rs:new] (fn)
- `run_hook` [manager.rs:run_hook] (fn)
- `manager.rs` [manager.rs] (and 1 additional symbols)
- `plugin_data_path` [plugin_data.rs:plugin_data_path] (fn)
- `RuntimePlugin` [runtime.rs:RuntimePlugin] (struct)
- `RuntimeContext` [runtime.rs:RuntimeContext] (struct)
- `discover` [runtime.rs:discover] (fn)
- `runtime.rs` [runtime.rs] (and 7 additional symbols)
- `TrustedKey` [signing.rs:TrustedKey] (struct)
- `Keyring` [signing.rs:Keyring] (struct)
- `new` [signing.rs:new] (fn)
- `signing.rs` [signing.rs] (and 4 additional symbols)
- [addon](./addon/README.md) - Child contract for `addon`
- [events](./events/README.md) - Child contract for `events`
- [filters](./filters/README.md) - Child contract for `filters`
- [marketplace](./marketplace/README.md) - Child contract for `marketplace`
- [router](./router/README.md) - Child contract for `router`
- [sandbox](./sandbox/README.md) - Child contract for `sandbox`
- [ui](./ui/README.md) - Child contract for `ui`

## Invariants
- [discovery.rs] Validates and sanitizes identifiers against path traversal and injection
- [manager.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [executor.rs] Reads or updates process environment variables
- [executor.rs] Sends `SIGKILL` to a timed-out plugin's process group
- [install.rs] Spawns external child processes via `std::process::Command`
- [install.rs] Mutates local filesystem (creates/modifies/removes files)
- [lockfile.rs] Mutates local filesystem (creates/modifies/removes files)
- [manager.rs] Spawns external child processes via `std::process::Command`, each in its own process group
- [plugin_data.rs] Mutates local filesystem (creates/modifies/removes files)
- [runtime.rs] Spawns external child processes via `std::process::Command`, each in its own process group
- [runtime.rs] Applies resource limits in the child through `pre_exec`
- [signing.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-plugins`

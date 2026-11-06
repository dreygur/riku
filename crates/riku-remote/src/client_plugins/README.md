# client_plugins

Client-side plugins: local executables in `~/.riku/client-plugins/<command>`
that handle a command on the laptop instead of forwarding it to the server.

## Interface
- `get_client_plugin_path` [discovery.rs:get_client_plugin_path] (fn, crate-private)
- `client_plugin_exists` [discovery.rs:client_plugin_exists] (fn)
- `list_client_plugins` [discovery.rs:list_client_plugins] (fn)
- `is_executable` [execute.rs:is_executable] (fn, crate-private)
- `execute_plugin` [execute.rs:execute_plugin] (fn, crate-private): runs the plugin with server, app, command and extra arguments as `$1`, `$2`, `$3`, `$4+`
- `try_execute_client_plugin` [mod.rs:try_execute_client_plugin] (fn)
- `cmd_plugin_list` [mod.rs:cmd_plugin_list] (fn)
- `cmd_plugin_exists` [mod.rs:cmd_plugin_exists] (fn)

## Side Effects
- [discovery.rs] Reads `HOME` to find the plugin directory
- [execute.rs] Spawns the plugin as a child process via `std::process::Command`

## Verification
- Run: `cargo test -p riku-remote`

# riku-remote (src)

## Modules

- `client_plugins`: discovery and execution of client-side plugins. See [client_plugins](./client_plugins/README.md).
- `plugin_author`: plugin author tooling, scaffolding and signing keys. See [plugin_author](./plugin_author/README.md).
- `plugin_cmd`: dispatch for the `plugin` command, which inspects client plugins (`run_plugin` [plugin_cmd.rs:run_plugin]).
- `quickstart`: the `riku quickstart` sample app scaffolding. See [quickstart](./quickstart/README.md).
- `remote`: the server list, command forwarding and the dashboard client. See [remote](./remote/README.md).
- `routing`: decides which commands a client plugin can intercept (`get_plugin_command` [routing.rs:get_plugin_command]) and builds the argument list passed to it (`build_plugin_args` [routing.rs:build_plugin_args]).

## Verification

- Run: `cargo test -p riku-remote`

# riku-client (src)

## Interface
- `main.rs`: the `rk` entry point (`run`, `main`); declares the `local` and `parse` modules
- `parse` [parse.rs:parse] (fn): turns argv into clap matches with the client's own help
- `no_local_mode` [local.rs:no_local_mode] (fn)
- `check_supported` [local.rs:check_supported] (fn)
- `run_without_server` [local.rs:run_without_server] (fn): runs the laptop side commands (`server`, `plugin`, `quickstart`, plugin authoring) from riku-remote, and refuses the rest when no server is selected

## Verification
- Run: `cargo test -p riku-client`

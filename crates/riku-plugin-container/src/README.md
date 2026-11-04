# riku-plugin-container (src)

## Overview
Component package located at `crates/riku-plugin-container/src`.

## Interface
- `compose_cmd` [compose_cli.rs:compose_cmd] (fn)
- `run_compose` [compose_cli.rs:run_compose] (fn)
- `pull_and_build` [compose_cli.rs:pull_and_build] (fn)
- `main.rs` [main.rs] (binary entry point: runtime plugin for `Dockerfile`, `Containerfile` and compose apps, subcommands `detect`, `build`, `env`, `start`, `pull-service`, `destroy`)

## Invariants
- None

## Side Effects
- [compose_cli.rs] Spawns external child processes via `std::process::Command`
- [main.rs] Spawns external child processes via `std::process::Command`
- [main.rs] Reads or updates process environment variables

## Verification
- Run: `cargo test -p riku-plugin-container`

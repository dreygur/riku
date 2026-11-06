# riku-plugin-clojure (src)

## Overview
Component package located at `crates/riku-plugin-clojure/src`.

## Interface
- `main.rs` [main.rs] (binary entry point: runtime plugin for Leiningen and Clojure CLI apps, subcommands `detect`, `build`, `env`, `start`)

## Invariants
- None

## Side Effects
- [main.rs] Spawns external child processes via `std::process::Command`
- [main.rs] Reads or updates process environment variables

## Verification
- Run: `cargo test -p riku-plugin-clojure`

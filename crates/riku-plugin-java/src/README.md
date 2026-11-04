# riku-plugin-java (src)

## Overview
Component package located at `crates/riku-plugin-java/src`.

## Interface
- `main.rs` [main.rs] (binary entry point: runtime plugin for Maven and Gradle apps, subcommands `detect`, `build`, `env`, `start`)

## Invariants
- None

## Side Effects
- [main.rs] Spawns external child processes via `std::process::Command`
- [main.rs] Reads or updates process environment variables

## Verification
- Run: `cargo test -p riku-plugin-java`

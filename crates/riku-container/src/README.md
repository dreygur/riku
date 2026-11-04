# riku-container (src)

## Overview
Component package located at `crates/riku-container/src`.

## Interface
- `compose` [lib.rs:compose] (module)
- `engine` [lib.rs:engine] (module)
- `types` [lib.rs:types] (module)
- [compose](./compose/README.md) - Child contract for `compose`
- [engine](./engine/README.md) - Child contract for `engine`
- [types](./types/README.md) - Child contract for `types`

## Invariants
- [types/names.rs] Name constructors enforce `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, so a name cannot start with `-` or carry a path separator or whitespace

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-container`

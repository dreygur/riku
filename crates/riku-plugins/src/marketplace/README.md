# marketplace

## Overview
Component package located at `crates/riku-plugins/src/marketplace`.

## Interface
- `MarketplaceEntry` [index.rs:MarketplaceEntry] (struct)
- `read_index` [index.rs:read_index] (fn)
- `MarketplaceService` [mod.rs:MarketplaceService] (struct)
- `new` [mod.rs:new] (fn)
- `add` [mod.rs:add] (fn)
- `mod.rs` [mod.rs] (and 6 additional symbols)
- `MarketplaceRecord` [registry.rs:MarketplaceRecord] (struct)
- `MarketplaceRegistry` [registry.rs:MarketplaceRegistry] (struct)
- `new` [registry.rs:new] (fn)
- `registry.rs` [registry.rs] (and 5 additional symbols)

## Invariants
- [mod.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [mod.rs] Spawns external child processes via `std::process::Command`
- [mod.rs] Mutates local filesystem (creates/modifies/removes files)
- [registry.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-plugins`

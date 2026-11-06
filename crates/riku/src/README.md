# riku (src)

## Overview
Component package located at `crates/riku/src`.

## Interface
- `riku_cli as cli` [lib.rs:riku_cli as cli] (re-export)
- `riku_config as config` [lib.rs:riku_config as config] (re-export)
- `riku_dashboard as dashboard` [lib.rs:riku_dashboard as dashboard] (re-export)
- `lib.rs` [lib.rs] (and 6 additional symbols)
- `main.rs` [main.rs] (internal module definitions)

## Invariants
- None

## Side Effects
- [main.rs] Reads process environment variables (`SSH_ORIGINAL_COMMAND`)

## Verification
- Run: `cargo test -p riku`

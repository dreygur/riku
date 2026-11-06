# riku-bundle (src)

## Overview
Component package located at `crates/riku-bundle/src`.

## Interface
- `RIKU_PLUGIN_API` [lib.rs:RIKU_PLUGIN_API] (const)
- `ROUTER_PROTOCOL` [lib.rs:ROUTER_PROTOCOL] (const)
- `manifest` [lib.rs:manifest] (module)
- `lib.rs` [lib.rs] (and 4 additional symbols)
- `PluginType` [manifest.rs:PluginType] (enum)
- `SubscribeMode` [manifest.rs:SubscribeMode] (enum)
- `Capabilities` [manifest.rs:Capabilities] (struct)
- `manifest.rs` [manifest.rs] (and 10 additional symbols)
- `RouterSection` [router_section.rs:RouterSection] (struct)
- `Keypair` [signing.rs:Keypair] (struct)
- `generate` [signing.rs:generate] (fn)
- `from_secret_hex` [signing.rs:from_secret_hex] (fn)
- `signing.rs` [signing.rs] (and 5 additional symbols)

## Invariants
- [manifest.rs] `PluginManifest::validate` rejects an unsupported API version, an empty or unsafe `name`, and any `entry` that is absolute or escapes the bundle via `..`
- [signing.rs] A valid signature covers only the entry bytes and says nothing about who holds the key; trust decisions stay with the caller

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-bundle`

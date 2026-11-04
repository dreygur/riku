# riku-caddy (src)

## Overview
Component package located at `crates/riku-caddy/src`.

## Interface
- `SYSTEM_CADDYFILE` [host.rs:SYSTEM_CADDYFILE] (const)
- `import_line` [host.rs:import_line] (fn)
- `has_import` [host.rs:has_import] (fn)
- `HttpsPolicy` [lib.rs:HttpsPolicy] (re-export)
- `site_path` [site.rs:site_path] (fn)
- `render_site` [site.rs:render_site] (fn)
- `write_site` [site.rs:write_site] (fn)
- `site.rs` [site.rs] (and 1 additional symbols)

## Invariants
- [site.rs] Only typed values reach a site file: validated `Domain`s, typed `Backend`s, and an app name validated before deploy reaches this point; no raw ENV string is written

## Side Effects
- [site.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-caddy`

# dump_state

## Overview
Component package located at `crates/riku-cli/src/apps/dump_state`.

## Interface
- `RouteState` [route.rs:RouteState] (enum)
- `route_state` [route.rs:route_state] (fn)
- `load_app_stats` [supervisor_files.rs:load_app_stats] (fn)
- `supervisor_uptime_seconds` [supervisor_files.rs:supervisor_uptime_seconds] (fn)
- `unix_timestamp_now` [supervisor_files.rs:unix_timestamp_now] (fn)

## Invariants
- None

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-cli`

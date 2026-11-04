# process

## Overview
Component package located at `crates/riku-cli/src/apps/process`.

## Interface
- `cmd_ps_all` [mod.rs:cmd_ps_all] (re-export)
- `cmd_ps_scale` [mod.rs:cmd_ps_scale] (re-export)
- `cmd_ps_show` [mod.rs:cmd_ps_show] (re-export)
- `load_stats` [helpers.rs:load_stats] (fn)
- `lookup_process_stats` [helpers.rs:lookup_process_stats] (fn)
- `read_worker_config` [helpers.rs:read_worker_config] (fn)
- `helpers.rs` [helpers.rs] Process stats lookup and worker config file helpers, re-exported to the sibling modules through `ps_all.rs`
- `cmd_ps_all` [ps_all.rs:cmd_ps_all] (fn)
- `cmd_ps_scale` [ps_scale.rs:cmd_ps_scale] (fn)
- `container_rows_for_app` [ps_show.rs:container_rows_for_app] (fn)
- `cmd_ps_show` [ps_show.rs:cmd_ps_show] (fn)
- `show_desired_scale_verbose` [scaling_display.rs:show_desired_scale_verbose] (fn)
- `show_scaling_compact` [scaling_display.rs:show_scaling_compact] (fn)
- `scaling_display.rs` [scaling_display.rs] Shows a stopped app's scaling configuration from its SCALING file

## Invariants
- None

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-cli`

# cron

## Overview
Component package located at `crates/riku-util/src/cron`.

## Interface
- `CronJob` [mod.rs:CronJob] (struct)
- `new` [mod.rs:new] (fn)
- `should_run_now` [mod.rs:should_run_now] (fn)
- `mod.rs` [mod.rs] (and 8 additional symbols)
- `calculate_next_run` [parser.rs:calculate_next_run] (fn)
- `calculate_next_run_after` [parser.rs:calculate_next_run_after] (fn)
- `validate_cron_expression` [parser.rs:validate_cron_expression] (fn)

## Invariants
- [parser.rs] `validate_cron_expression` rejects a malformed schedule; all schedules are read in UTC

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-util`

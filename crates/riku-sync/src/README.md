# riku-sync (src)

## Overview
Component package located at `crates/riku-sync/src`.

## Interface
- `apply` [apply.rs:apply] (fn)
- `apply_locked` [apply.rs:apply_locked] (fn)
- `write_bundle` [bundle.rs:write_bundle] (fn)
- `Budget` [capped.rs:Budget] (struct)
- `new` [capped.rs:new] (fn)
- `allow` [capped.rs:allow] (fn)
- `capped.rs` [capped.rs] (and 3 additional symbols)
- `wanted_sizes` [check.rs:wanted_sizes] (fn)
- `ensure_real_dirs` [guard.rs:ensure_real_dirs] (fn)
- `real_file` [guard.rs:real_file] (fn)
- `apply` [lib.rs:apply] (module)
- `bundle` [lib.rs:bundle] (module)
- `limits` [lib.rs:limits] (module)
- `lib.rs` [lib.rs] (and 10 additional symbols)
- `DEFAULT_MAX_NEEDED_BYTES` [limits.rs:DEFAULT_MAX_NEEDED_BYTES] (const)
- `DEFAULT_FREE_MARGIN_BYTES` [limits.rs:DEFAULT_FREE_MARGIN_BYTES] (const)
- `ReceiveLimits` [limits.rs:ReceiveLimits] (struct)
- `limits.rs` [limits.rs] (and 2 additional symbols)
- `LineError` [line.rs:LineError] (enum)
- `fixed_text` [line.rs:fixed_text] (fn)
- `read_line` [line.rs:read_line] (fn)
- `LOCK_SUFFIX` [lock.rs:LOCK_SUFFIX] (const)
- `SyncLock` [lock.rs:SyncLock] (struct)
- `SyncBusy` [lock.rs:SyncBusy] (struct)
- `lock.rs` [lock.rs] (and 1 additional symbols)
- `apply_dir_modes` [modes.rs:apply_dir_modes] (fn)
- `chmod_dir` [modes.rs:chmod_dir] (fn)
- `plan_transfer` [plan.rs:plan_transfer] (fn)
- `receive_bundle` [receive.rs:receive_bundle] (fn)
- `scan` [scan.rs:scan] (fn)
- `hash_file` [scan.rs:hash_file] (fn)
- `send` [send.rs:send] (fn)
- `receive` [serve.rs:receive] (fn)
- `SYNC_PROTOCOL` [session.rs:SYNC_PROTOCOL] (const)
- `MAX_LINE_BYTES` [session.rs:MAX_LINE_BYTES] (const)
- `SendReport` [session.rs:SendReport] (struct)
- `session.rs` [session.rs] (and 5 additional symbols)
- `sync_over_sockets` [session_kit.rs:sync_over_sockets] (fn)
- `request_line` [session_kit.rs:request_line] (fn)
- `honest_client` [session_kit.rs:honest_client] (fn)
- `session_kit.rs` [session_kit.rs] (and 5 additional symbols)
- `Staging` [staging.rs:Staging] (struct)
- `create` [staging.rs:create] (fn)
- `root` [staging.rs:root] (fn)
- `staging.rs` [staging.rs] (and 6 additional symbols)
- `STAGING_SUFFIX` [swap.rs:STAGING_SUFFIX] (const)
- `sibling` [swap.rs:sibling] (fn)
- `remove_tree` [swap.rs:remove_tree] (fn)
- `swap.rs` [swap.rs] (and 2 additional symbols)
- `SETTLED` [testkit.rs:SETTLED] (const)
- `rel` [testkit.rs:rel] (fn)
- `put` [testkit.rs:put] (fn)
- `testkit.rs` [testkit.rs] (and 13 additional symbols)
- `RelPath` [wire.rs:RelPath] (struct)
- `as_str` [wire.rs:as_str] (fn)
- `to_path` [wire.rs:to_path] (fn)
- `wire.rs` [wire.rs] (and 7 additional symbols)

## Invariants
- [wire.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [limits.rs] Reads free disk space with `statvfs`
- [lock.rs] Takes an exclusive `flock` on a lock file opened with `O_NOFOLLOW`
- [modes.rs] Changes directory modes with `fchmod` on an opened descriptor
- [scan.rs] Reads file metadata without following symlinks
- [session_kit.rs] Connects a client and server over `UnixStream` pairs, test builds only
- [staging.rs] Mutates local filesystem (creates/modifies/removes files)
- [swap.rs] Mutates local filesystem (creates/modifies/removes files)
- [testkit.rs] Mutates local filesystem (creates/modifies/removes files), test builds only

## Verification
- Run: `cargo test -p riku-sync`

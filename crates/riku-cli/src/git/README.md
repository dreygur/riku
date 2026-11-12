# git

## Overview
Component package located at `crates/riku-cli/src/git`.

## Interface
- `cmd_git_hook` [hook.rs:cmd_git_hook] (fn)
- `hook` [mod.rs:hook] (module)
- `receive_pack` [mod.rs:receive_pack] (module)
- `repo` [mod.rs:repo] (module)
- `mod.rs` [mod.rs] (and 1 additional symbols)
- `cmd_git_receive_pack` [receive_pack.rs:cmd_git_receive_pack] (fn)
- `cmd_git_upload_pack` [receive_pack.rs:cmd_git_upload_pack] (fn)
- `post_receive_hook` [repo.rs:post_receive_hook] (fn)

## Invariants
- [hook.rs] Validates and sanitizes identifiers against path traversal and injection
- [receive_pack.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [hook.rs] Spawns external child processes via `std::process::Command`
- [hook.rs] Mutates local filesystem (creates/modifies/removes files)
- [receive_pack.rs] Spawns external child processes via `std::process::Command`
- [receive_pack.rs] Mutates local filesystem (creates/modifies/removes files)
- [link_repo.rs] Links `repos/<app>.git` to the pushed repo, only inside `repos/`

## Verification
- Run: `cargo test -p riku-cli`

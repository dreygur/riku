# sandbox

## Overview
Component package located at `crates/riku-plugins/src/sandbox`.

## Interface
- `Sandbox` [apply.rs:Sandbox] (struct)
- `from_capabilities` [apply.rs:from_capabilities] (fn)
- `harden` [apply.rs:harden] (fn)
- `install` [enforce.rs:install] (fn)
- `harden` [mod.rs:harden] (fn)
- `Sandbox` [mod.rs:Sandbox] (re-export)
- `SandboxPaths` [mod.rs:SandboxPaths] (re-export)
- `SandboxPaths` [paths.rs:SandboxPaths] (struct)
- `Resolved` [paths.rs:Resolved] (enum)
- `resolve` [paths.rs:resolve] (fn)

## Invariants
- None

## Side Effects
- [apply.rs] Attaches the sandbox to a child as a `pre_exec` hook
- [enforce.rs] In the forked child, sets `PR_SET_NO_NEW_PRIVS` and applies Landlock rules

## Verification
- Run: `cargo test -p riku-plugins`

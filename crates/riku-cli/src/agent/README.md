# agent

## Overview
Component package located at `crates/riku-cli/src/agent`.

## Interface
- `AuditEntry` [audit.rs:AuditEntry] (struct)
- `open_audit_log` [audit.rs:open_audit_log] (fn)
- `open_before_mutation` [audit.rs:open_before_mutation] (fn)
- `audit.rs` [audit.rs] (and 2 additional symbols)
- `AgentSession` [auth.rs:AgentSession] (struct)
- `from_flags` [auth.rs:from_flags] (fn)
- `check_rate_limit` [auth.rs:check_rate_limit] (fn)
- `auth.rs` [auth.rs] (and 1 additional symbols)
- `cmd_agent_execute` [execute.rs:cmd_agent_execute] (fn)
- `cmd_agent_help` [help.rs:cmd_agent_help] (fn)
- `audit` [mod.rs:audit] (module)
- `auth` [mod.rs:auth] (module)
- `commands` [mod.rs:commands] (module)
- `mod.rs` [mod.rs] (and 7 additional symbols)
- `cmd_agent_intro` [schema.rs:cmd_agent_intro] (fn)
- `cmd_agent_schema` [schema.rs:cmd_agent_schema] (fn)
- `Access` [state.rs:Access] (enum)
- `agent_dir` [state.rs:agent_dir] (fn)
- `open_private` [state.rs:open_private] (fn)
- `AgentScope` [types.rs:AgentScope] (enum)
- `parse` [types.rs:parse] (fn)
- `allows` [types.rs:allows] (fn)
- `types.rs` [types.rs] (and 8 additional symbols)
- [commands](./commands/README.md) - Child contract for `commands`

## Invariants
- [execute.rs] Checks the agent's rate limit and scope permission, and opens the audit log for mutating commands, before running a command

## Side Effects
- [auth.rs] Takes an exclusive `flock` on the rate limit state file
- [state.rs] Creates the agent state directory (`0700`) and files (`0600`), opening every file with `O_NOFOLLOW`

## Verification
- Run: `cargo test -p riku-cli`

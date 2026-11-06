# riku-args (src)

## Overview
Component package located at `crates/riku-args/src`.

## Interface
- `ServerName` [address.rs:ServerName] (struct)
- `as_str` [address.rs:as_str] (fn)
- `SshAddress` [address.rs:SshAddress] (struct)
- `address.rs` [address.rs] (and 3 additional symbols)
- `apply_app_hint` [app_hint.rs:apply_app_hint] (fn)
- `Cli` [cli.rs:Cli] (struct)
- `require_app` [cli.rs:require_app] (fn)
- `Commands` [cli.rs:Commands] (enum)
- `for_client` [client_text.rs:for_client] (fn)
- `AppsCmd` [cmds.rs:AppsCmd] (enum)
- `ConfigCmd` [cmds.rs:ConfigCmd] (enum)
- `StatsCmd` [cmds.rs:StatsCmd] (enum)
- `cmds.rs` [cmds.rs] (and 7 additional symbols)
- `ContainerSubCmd` [container_args.rs:ContainerSubCmd] (enum)
- `FORWARD_HEADING` [help.rs:FORWARD_HEADING] (const)
- `forward_heading` [help.rs:forward_heading] (fn)
- `command` [help.rs:command] (fn)
- `help.rs` [help.rs] (and 1 additional symbols)
- `address` [lib.rs:address] (module)
- `app_hint` [lib.rs:app_hint] (module)
- `cli` [lib.rs:cli] (module)
- `lib.rs` [lib.rs] (and 5 additional symbols)
- `command_names` [parse.rs:command_names] (fn)
- `parse_with_help` [parse.rs:parse_with_help] (fn)
- `DASHBOARD` [placement.rs:DASHBOARD] (const)
- `Placement` [placement.rs:Placement] (enum)
- `placement_of` [placement.rs:placement_of] (fn)
- `placement.rs` [placement.rs] (and 3 additional symbols)

## Invariants
- [address.rs] `ServerName` and `SshAddress` are built only through `TryFrom<&str>`. An `SshAddress` has exactly one `@`, and its user and host match a grammar ssh cannot read as an option. Values from the TOML file go through the same check as typed ones
- [help.rs] Command descriptions are sanitized before they are spliced into the clap help template, which has no escape for `{`
- [lib.rs] Depends on no server crate, so the laptop client can parse commands without linking them

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-args`

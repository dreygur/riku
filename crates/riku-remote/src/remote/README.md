# remote

The laptop side list of named riku servers, and forwarding commands to them.
`types` holds the in-memory list, `store` persists it, `git_remote` and
`target` pick the server for a command, `forward` runs it over ssh, `route`
ties those together for the binary, `flow` is the dispatch both `riku` and
`rk` run once the target is known, and `dashboard` opens a server's embedded
dashboard in the local browser.

## Interface
- `Outcome` [flow.rs:Outcome] (enum), `run_or_forward` [flow.rs:run_or_forward] (fn)
- `server_arg_positions`, `strip_target_args`, `needs_tty`, `ssh_args`, `run` [forward.rs] (fn)
- `RemoteRef` [git_remote.rs:RemoteRef] (struct), `parse_remotes`, `read_remotes` [git_remote.rs] (fn)
- `is_plugin_authoring`, `target_for`, `implied_server`, `forward` [route.rs] (fn)
- `run_at`, `run` [server_cmd.rs] (fn): `riku server add|list|remove|default`
- `servers_path`, `servers_path_from`, `load`, `save` [store.rs] (fn)
- `Source`, `Target` [target.rs] (enum), `Selection` [target.rs] (struct), `registered_names`, `resolve` [target.rs] (fn)
- `Server`, `ServerList` [types.rs] (struct); `ServerName` and `SshAddress` re-exported from `riku_args::address`
- [dashboard](./dashboard/README.md)

## Invariants
- [types.rs, server_cmd.rs, store.rs] Server names and ssh addresses go through `ServerName` and `SshAddress` before they are stored, and every entry is revalidated on load
- [forward.rs] The ssh destination follows `--`, and the remote command is a single argument built with `shlex::try_join`; no local shell is involved
- [git_remote.rs] Remote URLs are only compared against registered servers, so a repo's git config cannot add a new ssh target
- [route.rs] When `SSH_ORIGINAL_COMMAND` is set (riku running as the ssh forced command), the server list and git remotes are not read

## Side Effects
- [forward.rs] Spawns the system `ssh` via `std::process::Command`
- [git_remote.rs] Runs `git remote -v` with stdin closed and stderr discarded
- [store.rs] Reads `XDG_CONFIG_HOME` and `HOME`; writes `${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml` (directory 700, file 600) through a temp file and rename

## Verification
- Run: `cargo test -p riku-remote`

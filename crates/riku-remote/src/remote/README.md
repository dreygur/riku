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
- `RemoteRef` [git_remote.rs:RemoteRef] (struct), `parse_remotes`, `read_remotes`, `read_remotes_in` [git_remote.rs] (fn)
- `key_path`, `read_public_key`, `install_key` [key_install.rs] (fn): authorize this machine's key through the admin login
- `RemoteOutcome` [remote_add.rs] (enum), `add_git_remote` [remote_add.rs] (fn): add the repo's `riku` remote
- `chosen_key`, `finish_add` [server_add.rs] (fn): the steps of `server add` after the entry is saved
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
- [key_install.rs] Only the key type and data are sent, as one `shlex::try_join` argument of `ssh -t <admin> sudo riku deploy-key add`; stdin stays free for sudo
- [remote_add.rs] The app name in the added remote is the repo directory name after `validate_app_name`; an existing `riku` remote or one pointing at the server is kept
- [route.rs] When `SSH_ORIGINAL_COMMAND` is set (riku running as the ssh forced command), the server list and git remotes are not read

## Side Effects
- [forward.rs] Spawns the system `ssh` via `std::process::Command`
- [git_remote.rs] Runs `git remote -v` with stdin closed and stderr discarded
- [key_install.rs] Runs `ssh -t` to the admin login with inherited stdio
- [remote_add.rs] Runs `git rev-parse` and `git remote add` in the repo
- [store.rs] Reads `XDG_CONFIG_HOME` and `HOME`; writes `${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml` (directory 700, file 600) through a temp file and rename

## Verification
- Run: `cargo test -p riku-remote`

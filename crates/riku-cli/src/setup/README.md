# setup

## Overview
Component package located at `crates/riku-cli/src/setup`.

## Interface
- `install_root_binary` [binary.rs:install_root_binary] (fn, installs the running binary as the root owned `/usr/local/bin/riku`)
- `require_root_binary` [binary.rs:require_root_binary] (fn, a plain init needs that file to exist)
- `setup_caddy_host` [caddy_host.rs:setup_caddy_host] (fn)
- `BRANCH_ENV` [default_branch.rs:BRANCH_ENV] (const)
- `set_default_branch` [default_branch.rs:set_default_branch] (fn)
- `AccountName` [deploy_account.rs:AccountName] (struct)
- `as_str` [deploy_account.rs:as_str] (fn)
- `ensure_account` [deploy_account.rs:ensure_account] (fn)
- `read_key_file` [deploy_keys.rs:read_key_file] (fn)
- `parse_key_args` [deploy_keys.rs:parse_key_args] (fn)
- `add_deploy_keys` [deploy_keys.rs:add_deploy_keys] (fn)
- `DeployUser` [deploy_user.rs:DeployUser] (struct)
- `running_as_root` [deploy_user.rs:running_as_root] (fn)
- `deploy_user_name` [deploy_user.rs:deploy_user_name] (fn)
- `passwd_home` [deploy_user.rs:passwd_home] (fn)
- `lookup` [deploy_user.rs:lookup] (fn)
- `riku_paths` [deploy_user.rs:riku_paths] (fn)
- `run_relayed` [deploy_user.rs:run_relayed] (fn)
- `deploy_user_files.rs` [deploy_user_files.rs] `DeployUser` methods (`create_dir`, `chmod`, `read_file`) for the changes root init makes inside the deploy user's tree, each run as that user so a planted symlink can only lead where the user could already go
- `create_git_hook` [git_hook.rs:create_git_hook] (fn)
- `InstallScope` [guidance.rs:InstallScope] (enum)
- `print_finish` [guidance.rs:print_finish] (fn)
- `InitOptions` [init.rs:InitOptions] (struct)
- `cmd_init` [init.rs:cmd_init] (fn)
- `persist_router` [init_router.rs:persist_router] (fn)
- `deploy_router` [init_router.rs:deploy_router] (fn)
- `setup_router_host` [init_router.rs:setup_router_host] (fn)
- `binary` [mod.rs:binary] (module)
- `git_hook` [mod.rs:git_hook] (module)
- `init` [mod.rs:init] (module)
- `mod.rs` [mod.rs] (the other module declarations and the `cmd_init`/`InitOptions` re-export)
- `setup_nginx_host` [nginx_host.rs:setup_nginx_host] (fn) Root-only nginx host setup: installs the one root-owned include `/etc/nginx/conf.d/riku.conf`
- `warn_not_root` [prerequisites.rs:warn_not_root] (fn)
- `check_prerequisites` [prerequisites.rs:check_prerequisites] (fn)
- `root_init` [root_init.rs:root_init] (fn)
- `router_unit_after` [router_units.rs:router_unit_after] (fn)
- `unit_ordering` [router_units.rs:unit_ordering] (fn)
- `install_router_helper` [helper_install.rs:install_router_helper] (fn)
- `setup_ssh_key_interactive` [ssh.rs:setup_ssh_key_interactive] (fn)
- `SYSTEMD_DIR` [system_service.rs:SYSTEMD_DIR] (const)
- `install_systemd_service` [system_service.rs:install_systemd_service] (fn)
- `install_systemd_service` [systemd.rs:install_systemd_service] (re-export)
- `setup_systemd_service` [systemd.rs:setup_systemd_service] (re-export)
- `setup_systemd_service` [user_service.rs:setup_systemd_service] (fn)

## Invariants
- [deploy_keys.rs] Keeps only validated `PublicKey`s read from `--ssh-key`
- [root_init.rs] Passes on only keys `deploy_keys.rs` read and validated as root
- [deploy_user_files.rs] Every change inside the deploy user's tree runs as that user, never as root
- [nginx_host.rs] Writes the include only into a root-owned directory no one else can write

## Side Effects
- [binary.rs] Mutates local filesystem (creates/modifies/removes files)
- [binary.rs] Reads process environment variables
- [caddy_host.rs] Spawns external child processes via `std::process::Command`
- [default_branch.rs] Spawns external child processes via `std::process::Command`
- [deploy_account.rs] Spawns external child processes via `std::process::Command`
- [deploy_keys.rs] Spawns external child processes via `std::process::Command`
- [deploy_user.rs] Spawns external child processes as the deploy user, each in a new session (`setsid`) with a filtered environment
- [deploy_user.rs] Reads process environment variables
- [git_hook.rs] Mutates local filesystem (creates/modifies/removes files)
- [guidance.rs] Spawns external child processes via `std::process::Command`
- [init.rs] Mutates local filesystem (creates/modifies/removes files)
- [init.rs] Reads process environment variables
- [init_router.rs] Mutates local filesystem (creates/modifies/removes files)
- [init_router.rs] Reads process environment variables
- [nginx_host.rs] Writes the include file under `/etc/nginx` (as root)
- [prerequisites.rs] Spawns external child processes via `std::process::Command`
- [root_init.rs] Spawns external child processes and mutates local filesystem
- [router_units.rs] Mutates local filesystem (creates/modifies/removes files)
- [ssh.rs] Spawns external child processes via `std::process::Command`
- [ssh.rs] Mutates local filesystem (creates/modifies/removes files)
- [system_service.rs] Spawns external child processes via `std::process::Command`
- [system_service.rs] Mutates local filesystem (creates/modifies/removes files)
- [user_service.rs] Spawns external child processes via `std::process::Command`
- [user_service.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-cli`

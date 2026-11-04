# riku-util (src)

## Overview
Component package located at `crates/riku-util/src`.

## Interface
- `RIKU_KEY_MARKER` [authorized_keys.rs:RIKU_KEY_MARKER] (const)
- `migrate_line` [authorized_keys.rs:migrate_line] (fn)
- `migrate_file` [authorized_keys.rs:migrate_file] (fn)
- `authorized_keys.rs` [authorized_keys.rs] (and 3 additional symbols)
- `Backend` [backend.rs:Backend] (enum)
- `loopback` [backend.rs:loopback] (fn)
- `DeployLogger` [deploy_logger.rs:DeployLogger] (struct)
- `new` [deploy_logger.rs:new] (fn)
- `log` [deploy_logger.rs:log] (fn)
- `deploy_logger.rs` [deploy_logger.rs] (and 3 additional symbols)
- `format_table` [display.rs:format_table] (fn)
- `print_table` [display.rs:print_table] (fn)
- `print_table_with_title` [display.rs:print_table_with_title] (fn)
- `display.rs` [display.rs] (and 9 additional symbols)
- `Domain` [domain.rs:Domain] (struct)
- `as_str` [domain.rs:as_str] (fn)
- `parse_domains` [domain.rs:parse_domains] (fn)
- `expandvars` [env.rs:expandvars] (fn)
- `parse_settings` [env.rs:parse_settings] (fn)
- `write_config` [env.rs:write_config] (fn)
- `is_on` [flag.rs:is_on] (fn)
- `https_only` [flag.rs:https_only] (fn)
- `https_only_warning` [flag.rs:https_only_warning] (fn)
- `write_atomic` [fs.rs:write_atomic] (fn)
- `copy_dir_recursive` [fs.rs:copy_dir_recursive] (fn)
- `count_files` [fs.rs:count_files] (fn)
- `RENAMED_KEYS` [legacy_env.rs:RENAMED_KEYS] (const)
- `legacy_keys` [legacy_env.rs:legacy_keys] (fn)
- `authorized_keys` [lib.rs:authorized_keys] (module)
- `backend` [lib.rs:backend] (module)
- `cron` [lib.rs:cron] (module)
- `lib.rs` [lib.rs] (and 21 additional symbols)
- `validate_env_vars` [nginx_validation.rs:validate_env_vars] (fn)
- `print_env_warnings` [nginx_validation.rs:print_env_warnings] (fn)
- `get_free_port` [process_util.rs:get_free_port] (fn)
- `validate_node_version` [process_util.rs:validate_node_version] (fn)
- `ENVVAR_RE` [procfile.rs:ENVVAR_RE] (static)
- `parse_procfile` [procfile.rs:parse_procfile] (fn)
- `PublicKey` [public_key.rs:PublicKey] (struct)
- `blob` [public_key.rs:blob] (fn)
- `parse_public_keys` [public_key.rs:parse_public_keys] (fn)
- `HttpsPolicy` [route_plan.rs:HttpsPolicy] (enum)
- `RoutePlan` [route_plan.rs:RoutePlan] (struct)
- `constant_time_eq` [secure.rs:constant_time_eq] (fn)
- `spawn_retrying_etxtbsy` [spawn.rs:spawn_retrying_etxtbsy] (fn)
- `output_retrying_etxtbsy` [spawn.rs:output_retrying_etxtbsy] (fn)
- `setup_authorized_keys` [ssh_keys.rs:setup_authorized_keys] (fn)
- `generate_token` [token.rs:generate_token] (fn)
- `ensure_path_within` [validation.rs:ensure_path_within] (fn)
- `sanitize_app_name` [validation.rs:sanitize_app_name] (fn)
- `validate_app_name` [validation.rs:validate_app_name] (fn)
- `validation.rs` [validation.rs] (and 1 additional symbols)
- [cron](./cron/README.md) - Child contract for `cron`
- [resource_limits](./resource_limits/README.md) - Child contract for `resource_limits`

## Invariants
- [deploy_logger.rs] Validates and sanitizes identifiers against path traversal and injection
- [lib.rs] Validates and sanitizes identifiers against path traversal and injection
- [nginx_validation.rs] Validates and sanitizes identifiers against path traversal and injection
- [process_util.rs] Validates and sanitizes identifiers against path traversal and injection
- [procfile.rs] Validates and sanitizes identifiers against path traversal and injection
- [public_key.rs] Validates and sanitizes identifiers against path traversal and injection
- [validation.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [deploy_logger.rs] Mutates local filesystem (creates/modifies/removes files)
- [env.rs] Mutates local filesystem (creates/modifies/removes files)
- [fs.rs] Mutates local filesystem (creates/modifies/removes files)
- [process_util.rs] Performs network I/O or binds socket
- [ssh_keys.rs] Mutates local filesystem (creates/modifies/removes files)
- [authorized_keys.rs] Rewrites `authorized_keys` by renaming a mode 600 sibling over it; refuses to run as root
- [ssh_keys.rs] Reads `HOME` from the process environment

## Verification
- Run: `cargo test -p riku-util`

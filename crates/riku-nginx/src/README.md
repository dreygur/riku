# riku-nginx (src)

## Overview
Component package located at `crates/riku-nginx/src`.

## Interface
- `generate_acme_nginx_config` [acme.rs:generate_acme_nginx_config] (fn)
- `ACME_CONFIG_FILE` [acme.rs:ACME_CONFIG_FILE] (const)
- `read_app_snippet` [app_snippet.rs:read_app_snippet] (fn)
- `insert_include_file` [app_snippet.rs:insert_include_file] (fn)
- `upstream_port` [backends.rs:upstream_port] (fn)
- `upstream_name` [backends.rs:upstream_name] (fn)
- `socket_path` [backends.rs:socket_path] (fn)
- `bind_address` [backends.rs:bind_address] (fn)
- `upstream_port_number` [backends.rs:upstream_port_number] (fn)
- `default_backends` [backends.rs:default_backends] (fn)
- `generate_cloudflare_ips_config` [cloudflare.rs:generate_cloudflare_ips_config] (fn)
- `insert_address_context` [context.rs:insert_address_context] (fn)
- `insert_feature_flags` [context.rs:insert_feature_flags] (fn)
- `insert_portmap_context` [context.rs:insert_portmap_context] (fn)
- `build_context` [context.rs:build_context] (fn)
- `insert_cache_context` [context_cache.rs:insert_cache_context] (fn)
- `include_line` [host.rs:include_line] (fn)
- `include_file_contents` [host.rs:include_file_contents] (fn)
- `write_include_file` [host.rs:write_include_file] (fn)
- `includes_riku_sites` [host.rs:includes_riku_sites] (fn)
- `check_host_ready` [host_check.rs:check_host_ready] (fn)
- `stray_site_configs` [host_check.rs:stray_site_configs] (fn)
- `HostReadiness` [host_check.rs:HostReadiness] (enum)
- `install_site` [install.rs:install_site] (fn)
- `install_checked` [install.rs:install_checked] (fn)
- `generate_nginx_config` [lib.rs:generate_nginx_config] (fn)
- `reload_nginx` [reload.rs:reload_nginx] (fn)
- `reload_nginx_with` [reload.rs:reload_nginx_with] (fn)
- `remove_nginx_files` [remove.rs:remove_nginx_files] (fn)
- `NginxRemoval` [remove.rs:NginxRemoval] (struct)
- `sanitize_nginx_value` [sanitize.rs:sanitize_nginx_value] (fn)
- `sanitize_env_for_nginx` [sanitize.rs:sanitize_env_for_nginx] (fn)
- `statements` [snippet_grammar.rs:statements] (fn)
- `ensure_ssl_certificates` [ssl.rs:ensure_ssl_certificates] (fn)
- `generate_nginx_config_from_template` [template.rs:generate_nginx_config_from_template] (fn)
- `render_nginx_config` [template.rs:render_nginx_config] (fn)
- `TempFile` [validate.rs:TempFile] (struct)
- `validate_nginx_config` [validate.rs:validate_nginx_config] (fn)
- `test_support.rs` holds fixtures shared by the crate's test modules

## Modules
- `acme.rs`: the first-run ACME challenge config, written under a name that does not end in `.conf` so the host include never loads it
- `app_snippet.rs`: the app's own snippet named by `RIKU_NGINX_INCLUDE_FILE`, checked for path, grammar and directive allowlist, and checked again after the `nginx.include_content` filter chain
- `context_cache.rs`: the proxy cache section of the template context (zone size, cache times, cache path)
- `host.rs`: the one root-owned include file, `nginx_include_file`, holding `include <nginx_root>/*.conf;`
- `host_check.rs`: refuses a router install while `nginx_root` holds a `*.conf` that is no app's config; deploy and router apply check this first, and `riku doctor` reports it
- `install.rs`: writes a config as `<name>.conf.new`, checks it with `nginx -t`, and renames it into place only when nginx accepts it
- `remove.rs`: removes an app's files from `nginx_root` and nothing outside it
- `snippet_grammar.rs`: the strict grammar an app snippet must fit and its split into statements

## Invariants
- [sanitize.rs] Refuses `;`, `{`, `}` and newlines in values bound for nginx templates, plus backtick, `$`, quotes and `#` as defense in depth
- [template.rs] Sanitizes the app ENV and backend addresses before rendering
- [app_snippet.rs] The snippet must resolve inside the app directory, fit the grammar in `snippet_grammar.rs`, and use only allowlisted directives, before and after filter plugins run
- [host.rs] The path written into the root-owned include file must be absolute and free of directive-breaking characters
- [install.rs] A config nginx rejects is deleted and the previous live config stays
- [lib.rs] An app with no backends is refused, and nginx configs shipped in the app directory are ignored with a warning

## Side Effects
- [cloudflare.rs] Mutates local filesystem (creates/modifies/removes files)
- [cloudflare.rs] Performs network I/O through `riku-http`
- [host.rs] Writes the include file atomically
- [install.rs] Writes, renames and removes site configs in `nginx_root`
- [remove.rs] Removes an app's files from `nginx_root`
- [reload.rs] Spawns external child processes via `std::process::Command`
- [ssl.rs] Spawns external child processes via `std::process::Command`
- [ssl.rs] Mutates local filesystem (creates/modifies/removes files)
- [validate.rs] Spawns external child processes via `std::process::Command`
- [validate.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-nginx`

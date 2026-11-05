//! How the system Caddy finds riku's sites: one `import` line in the
//! system Caddyfile pointing at riku's site directory, `/etc/riku/caddy`.

use riku_config::RikuPaths;

/// The Caddyfile the distribution's `caddy` package runs.
pub const SYSTEM_CADDYFILE: &str = "/etc/caddy/Caddyfile";

/// Where Caddy's admin API listens. A unix socket, so no local process can
/// reach it over TCP.
pub const ADMIN_ADDRESS: &str = "unix//run/caddy/admin.sock";

/// The line the system Caddyfile needs to serve riku's sites.
pub fn import_line(paths: &RikuPaths) -> String {
    format!("import {}/*.caddy", paths.caddy_root.display())
}

/// Whether `caddyfile` already carries `import_line` on a line of its own.
pub fn has_import(caddyfile: &str, import_line: &str) -> bool {
    caddyfile.lines().any(|line| line.trim() == import_line)
}

//! `riku doctor` checks for the host router named in `<riku root>/ROUTER`.
//!
//! Read-only like every doctor check: it runs `nginx -t` or
//! `caddy validate` and reads the nginx include file, sites-enabled or the
//! system Caddyfile, but changes nothing.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::caddy::{has_import, import_line, SYSTEM_CADDYFILE};
use crate::config::program::find_program;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::deploy::router::RELOAD_ERROR_FILE;
use crate::nginx::{app_named_links, check_host_ready, HostReadiness, RouteChange};
use crate::setup::system_service::SYSTEMD_DIR;

use super::Check;

/// The last non-empty stderr line of a failed validator run.
fn last_error_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("invalid configuration")
        .trim()
        .to_string()
}

/// Turn a validator run into a check. Without root the validator often
/// cannot read the config, which is not a config problem.
fn validator_check(name: &str, command: &str, run: std::io::Result<Output>) -> Check {
    match run {
        Ok(o) if o.status.success() => Check::ok(name, format!("{command} passed")),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).to_lowercase();
            if stderr.contains("permission denied") {
                Check::warn(
                    name,
                    "cannot validate without root, re-run: sudo riku doctor",
                )
            } else {
                Check::fail(name, format!("{command} failed: {}", last_error_line(&o)))
            }
        }
        Err(e) => Check::warn(name, format!("could not run {command}: {e}")),
    }
}

/// The installed `name`, searched the way riku spawns it, or `None`.
fn installed_program(name: &str) -> Option<PathBuf> {
    let path = find_program(name);
    path.is_file().then_some(path)
}

/// Whether nginx loads riku's sites, and whether links from an older riku
/// would load them twice.
fn nginx_include(paths: &RikuPaths) -> Check {
    match check_host_ready(paths, RouteChange::Install) {
        Ok(HostReadiness::Ready) => Check::ok(
            "nginx include",
            format!("{} loads riku sites", paths.nginx_include_file.display()),
        ),
        Ok(HostReadiness::NoNginx(reason)) => Check::warn("nginx include", reason),
        Err(e) => Check::fail("nginx include", e.to_string()),
    }
}

/// Warns about sites-enabled links named like an app's config that point
/// elsewhere. riku leaves them alone, but they may serve the app's domain.
fn app_named_sites(paths: &RikuPaths) -> Option<Check> {
    let links = app_named_links(paths);
    links.first().map(|first| {
        Check::warn(
            "nginx sites",
            format!(
                "{} link(s) in {} share a riku app's name, such as {}: check they do not serve the same domain",
                links.len(),
                paths.nginx_sites_enabled.display(),
                first.display()
            ),
        )
    })
}

fn nginx(paths: &RikuPaths) -> Vec<Check> {
    let Some(program) = installed_program("nginx") else {
        return vec![Check::warn(
            "nginx",
            "not found: web serving disabled (apt install nginx)",
        )];
    };
    let run = Command::new(&program).arg("-t").output();
    let mut checks = vec![
        Check::ok("nginx", format!("found at {}", program.display())),
        nginx_include(paths),
    ];
    checks.extend(app_named_sites(paths));
    checks.push(validator_check("nginx config", "nginx -t", run));
    checks
}

/// Whether `caddyfile` imports riku's site directory.
fn caddy_import(paths: &RikuPaths, caddyfile: &Path) -> Check {
    let line = import_line(paths);
    match fs::read_to_string(caddyfile) {
        Ok(text) if has_import(&text, &line) => Check::ok(
            "caddy import",
            format!("{} imports riku sites", caddyfile.display()),
        ),
        Ok(_) => Check::warn(
            "caddy import",
            format!("add this line to {}: {line}", caddyfile.display()),
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => Check::fail(
            "caddy import",
            format!("{} not found: apt install caddy", caddyfile.display()),
        ),
        Err(e) => Check::warn(
            "caddy import",
            format!("cannot read {}: {e}", caddyfile.display()),
        ),
    }
}

fn caddy(paths: &RikuPaths, caddyfile: &Path) -> Vec<Check> {
    let Some(program) = installed_program("caddy") else {
        return vec![Check::fail(
            "caddy",
            "not found: the Caddy router needs it (apt install caddy)",
        )];
    };
    let run = Command::new(&program)
        .args(["validate", "--adapter", "caddyfile", "--config"])
        .arg(caddyfile)
        .output();
    vec![
        Check::ok("caddy", format!("found at {}", program.display())),
        caddy_import(paths, caddyfile),
        validator_check("caddy config", "caddy validate", run),
    ]
}

/// Fails with the error of the last reload that did not confirm. The file
/// is removed by the next reload that does.
fn last_reload(paths: &RikuPaths) -> Option<Check> {
    match fs::read_to_string(paths.riku_root.join(RELOAD_ERROR_FILE)) {
        Ok(text) => Some(Check::fail("router reload", text.trim().to_string())),
        Err(_) => None,
    }
}

/// Warns about the Caddy watcher units older versions installed, which now
/// reload Caddy a second time on every change.
fn old_caddy_units(systemd_dir: &Path) -> Option<Check> {
    if !systemd_dir.join("riku-caddy.path").exists() {
        return None;
    }
    Some(Check::warn(
        "caddy units",
        "riku-caddy.path is obsolete, re-run: sudo riku setup",
    ))
}

/// Checks for the active router: nginx keeps its `nginx -t` check, Caddy
/// gets a binary, import line and `caddy validate` check, and a plugin
/// router is reported by name. An unreadable `ROUTER` file fails. A failed
/// last reload and leftover Caddy units are reported for any router.
pub fn router(paths: &RikuPaths) -> Vec<Check> {
    let mut checks = match Router::load(paths) {
        Ok(Router::Nginx) => nginx(paths),
        Ok(Router::Caddy) => caddy(paths, Path::new(SYSTEM_CADDYFILE)),
        Ok(Router::Plugin(name)) => vec![Check::ok("router", format!("plugin {name}"))],
        Err(e) => return vec![Check::fail("router", e.to_string())],
    };
    checks.extend(last_reload(paths));
    checks.extend(old_caddy_units(Path::new(SYSTEMD_DIR)));
    checks
}

#[cfg(test)]
#[path = "router_check_tests.rs"]
mod tests;

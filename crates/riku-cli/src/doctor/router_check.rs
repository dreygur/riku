//! `riku doctor` checks for the host router named in `<riku root>/ROUTER`.
//!
//! Read-only like every doctor check: it runs `nginx -t` or
//! `caddy validate` and reads the system Caddyfile, but changes nothing.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Output};

use crate::caddy::{has_import, import_line, SYSTEM_CADDYFILE};
use crate::config::router::Router;
use crate::config::RikuPaths;

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

fn nginx() -> Vec<Check> {
    if which::which("nginx").is_err() {
        return vec![Check::warn(
            "nginx",
            "not found: web serving disabled (apt install nginx)",
        )];
    }
    let run = Command::new("nginx").arg("-t").output();
    vec![
        Check::ok("nginx", "found on PATH"),
        validator_check("nginx config", "nginx -t", run),
    ]
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
    if which::which("caddy").is_err() {
        return vec![Check::fail(
            "caddy",
            "not found: the Caddy router needs it (apt install caddy)",
        )];
    }
    let run = Command::new("caddy")
        .args(["validate", "--adapter", "caddyfile", "--config"])
        .arg(caddyfile)
        .output();
    vec![
        Check::ok("caddy", "found on PATH"),
        caddy_import(paths, caddyfile),
        validator_check("caddy config", "caddy validate", run),
    ]
}

/// Checks for the active router: nginx keeps its `nginx -t` check, Caddy
/// gets a binary, import line and `caddy validate` check, and a plugin
/// router is reported by name. An unreadable `ROUTER` file fails.
pub fn router(paths: &RikuPaths) -> Vec<Check> {
    match Router::load(paths) {
        Ok(Router::Nginx) => nginx(),
        Ok(Router::Caddy) => caddy(paths, Path::new(SYSTEM_CADDYFILE)),
        Ok(Router::Plugin(name)) => vec![Check::ok("router", format!("plugin {name}"))],
        Err(e) => vec![Check::fail("router", e.to_string())],
    }
}

#[cfg(test)]
#[path = "router_check_tests.rs"]
mod tests;

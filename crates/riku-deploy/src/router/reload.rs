//! Confirmed router reloads.
//!
//! [`reload`] returns only after the router has loaded the new config, so a
//! caller can act on that (drain a copy, report success) without guessing.
//! nginx is confirmed by watching `riku-nginx-reload.service`, the path
//! activated unit that does the reload on a host with systemd. Caddy is
//! reloaded through its admin endpoint, a router plugin through its own
//! `reload` verb.
//!
//! Security: every program and argument is fixed. Nothing from an app's ENV
//! reaches a command line. The failure text is written to a file under the
//! riku root, which only the deploy user can write.

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::plugin::reload_plugin;
use crate::caddy::SYSTEM_CADDYFILE;
use crate::config::router::Router;
use crate::config::{ReloadPrograms, RikuPaths};
use crate::util::spawn::output_retrying_etxtbsy;

#[cfg(not(test))]
const RELOAD_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
const RELOAD_WAIT: Duration = Duration::from_secs(1);
const PROBE_INTERVAL: Duration = Duration::from_millis(500);
const NGINX_UNIT: &str = "riku-nginx-reload.service";
const UNIT_PROPERTIES: &str = "ExecMainExitTimestampMonotonic,ExecMainStatus,LoadState";
/// File under the riku root holding the last failed reload's error text.
pub const ERROR_FILE: &str = "router-reload.err";

/// What the router looked like before its config was written.
pub enum Baseline {
    /// nginx has the reload unit, which last exited at this monotonic time.
    Unit(u64),
    /// Nothing to compare against: reload the router directly.
    Direct,
}

struct UnitState {
    exited_at: u64,
    status: String,
}

fn run_systemctl(tools: &ReloadPrograms) -> Result<(String, UnitState)> {
    let output = match output_retrying_etxtbsy(
        Command::new(&tools.systemctl)
            .args(["show", "-p", UNIT_PROPERTIES])
            .arg(NGINX_UNIT),
    ) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("could not run systemctl: {e}")),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let exited_at: u64 = value("ExecMainExitTimestampMonotonic")
        .parse()
        .unwrap_or_default();
    let state = UnitState {
        exited_at,
        status: value("ExecMainStatus"),
    };
    Ok((value("LoadState"), state))
}

impl Baseline {
    /// Record the router's state. Call it before the config is written, so a
    /// reload that finishes quickly is still seen as newer.
    pub fn capture(router: &Router, tools: &ReloadPrograms) -> Baseline {
        if !matches!(router, Router::Nginx) {
            return Baseline::Direct;
        }
        match run_systemctl(tools) {
            Ok((load, state)) if load != "not-found" => Baseline::Unit(state.exited_at),
            Ok(_) => Baseline::Direct,
            Err(e) => {
                tracing::warn!("no reload unit to watch, reloading nginx directly: {e}");
                Baseline::Direct
            }
        }
    }
}

#[allow(clippy::question_mark)]
fn await_nginx_unit(tools: &ReloadPrograms, before: u64) -> Result<()> {
    let deadline = Instant::now() + RELOAD_WAIT;
    loop {
        let (_, state) = match run_systemctl(tools) {
            Ok(read) => read,
            Err(e) => return Err(e),
        };
        if state.exited_at > before {
            if state.status == "0" {
                return Ok(());
            }
            return Err(anyhow!(
                "nginx reload failed: {NGINX_UNIT} exited with status {}",
                state.status
            ));
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "nginx did not confirm the reload within {}s: {NGINX_UNIT} did not run",
                RELOAD_WAIT.as_secs()
            ));
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

fn reload_caddy(tools: &ReloadPrograms) -> Result<()> {
    let output = match output_retrying_etxtbsy(Command::new(&tools.caddy).args([
        "reload",
        "--config",
        SYSTEM_CADDYFILE,
        "--adapter",
        "caddyfile",
    ])) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("caddy reload failed: could not run caddy: {e}")),
    };
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "caddy reload failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn confirm(
    tools: &ReloadPrograms,
    router: &Router,
    paths: &RikuPaths,
    before: &Baseline,
) -> Result<()> {
    match (router, before) {
        (Router::Nginx, Baseline::Unit(before)) => await_nginx_unit(tools, *before),
        (Router::Nginx, Baseline::Direct) => crate::nginx::reload_nginx_with(&tools.nginx),
        (Router::Caddy, _) => reload_caddy(tools),
        (Router::Plugin(name), _) => reload_plugin(name, paths),
    }
}

fn record_outcome(paths: &RikuPaths, outcome: &Result<()>) {
    let file = paths.riku_root.join(ERROR_FILE);
    let written = match outcome {
        Ok(()) => match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        Err(cause) => {
            fs::create_dir_all(&paths.riku_root).and_then(|_| fs::write(&file, cause.to_string()))
        }
    };
    if let Err(e) = written {
        tracing::warn!("cannot update {}: {e}", file.display());
    }
}

/// Reload `router` and return once it has confirmed the new config.
///
/// `before` is the [`Baseline`] taken before the config was written. The
/// programs run come from `paths.reload_programs`. The last failure is also
/// written to `<riku root>/router-reload.err` for `riku doctor`, and the
/// file is removed on success.
pub fn reload(router: &Router, paths: &RikuPaths, before: &Baseline) -> Result<()> {
    let outcome = confirm(&paths.reload_programs, router, paths, before);
    record_outcome(paths, &outcome);
    outcome
}

#[cfg(test)]
#[path = "reload_tests.rs"]
mod tests;

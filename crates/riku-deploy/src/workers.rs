//! Generic worker configuration creation for deployed apps.
//!
//! Handles Procfile parsing and worker config generation.
//! Scaling delta logic lives in `super::scaling`.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use riku_container::types::CopyIndex;

use crate::config::RikuPaths;
use crate::util::{echo, parse_procfile};

pub(crate) use super::scaling::apply_scaling_deltas;

/// The process kind a runtime plugin's start command runs as.
const WEB_KIND: &str = "web";
/// Procfile entries the deploy runs once. As workers, the supervisor would
/// restart them every time they exit.
const ONE_SHOT_KINDS: [&str; 2] = ["release", "preflight"];

mod files;
mod pinned;
mod ports;
mod socket;
pub(crate) mod web_env;

use files::{remove_stale_configs, write_worker_config};
use pinned::check_pinned_port;
use socket::check_custom_socket;
use web_env::build_worker_env;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read the scaling count for a given process kind from the SCALING file.
///
/// Returns 1 if the file doesn't exist or the kind isn't listed.
pub fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    if !scaling_path.exists() {
        return Ok(1);
    }
    let content = fs::read_to_string(&scaling_path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            let val = line[pos + 1..].trim();
            if key == kind {
                if let Ok(n) = val.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(1)
}

/// Create worker configs for every process entry in the app's Procfile.
///
/// `start_cmd` is an optional fallback command supplied by the runtime plugin
/// via its `start` subcommand. It fills a Procfile entry with an empty
/// command, and runs as the one `web` process when there is no Procfile.
#[allow(clippy::question_mark)]
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    if let Err(e) = check_pinned_port(app, env, paths) {
        return Err(e);
    }
    if let Err(e) = check_custom_socket(app, env, paths) {
        return Err(e);
    }
    let mut taken = match enabled_ports(app, paths) {
        Ok(ports) => ports,
        Err(e) => return Err(e),
    };
    if should_restart(env) {
        remove_stale_configs(app, paths);
    }

    let entries = match (parse_procfile(&app_path.join("Procfile")), start_cmd) {
        (Ok(Some(e)), _) if !e.is_empty() => e,
        (Ok(_), Some(cmd)) => HashMap::from([(WEB_KIND.to_string(), cmd.to_string())]),
        (Ok(_), None) => {
            echo(
                "-----> No Procfile found, skipping process creation",
                "yellow",
            );
            return Ok(());
        }
        (Err(e), _) => return Err(e),
    };

    let processes = entries
        .iter()
        .filter(|(kind, _)| !ONE_SHOT_KINDS.contains(&kind.as_str()));
    for (kind, command) in processes {
        let effective_cmd = if command.is_empty() {
            start_cmd.unwrap_or(command.as_str())
        } else {
            command.as_str()
        };
        let count = match read_scaling_count(paths, app, kind) {
            Ok(count) => count,
            Err(e) => return Err(e),
        };
        for copy in (1..=count).filter_map(CopyIndex::new) {
            let i = copy.get();
            let worker_env = match build_worker_env(app, kind, copy, env, &taken, paths) {
                Ok(worker_env) => worker_env,
                Err(e) => return Err(e),
            };
            if let Some(port) = worker_env.get("PORT").and_then(|p| p.parse().ok()) {
                taken.push(port);
            }
            if let Err(e) =
                write_worker_config(app, app_path, kind, effective_cmd, i, worker_env, paths)
            {
                return Err(e);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Ports the app's enabled web copies run on. Old copies keep theirs until
/// the supervisor restarts them, so new copies must not be given the same.
fn enabled_ports(app: &str, paths: &RikuPaths) -> Result<Vec<u16>> {
    match crate::router::web_copies(app, paths) {
        Ok(copies) => Ok(copies
            .iter()
            .filter_map(|copy| copy.env.get("PORT").and_then(|port| port.parse().ok()))
            .collect()),
        Err(e) => Err(anyhow::anyhow!(
            "cannot read the web copies of '{app}': {e}"
        )),
    }
}

/// Returns true when `RIKU_AUTO_RESTART` is not explicitly disabled.
fn should_restart(env: &HashMap<String, String>) -> bool {
    env.get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true)
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;

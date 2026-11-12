//! An app's merged config: the env map and the process list every reader
//! uses, so `riku.toml`, the ENV file and the server override give one
//! answer wherever riku reads them.
//!
//! Env layers, later wins:
//!
//! ```text
//!   <app_root>/<app>/riku.toml      repo file (from the checkout)
//!   <env_root>/<app>/ENV            app env vars
//!   <env_root>/<app>/riku.toml      server override (riku settings)
//! ```
//!
//! riku reads its settings only from the two `riku.toml` files. An ENV line
//! whose key is a settings table key (`RIKU_DOMAINS` and the like) is
//! dropped, so it can neither set nor override a setting.
//!
//! Processes come from the repo `riku.toml` `[processes]`/`[cron]`, else the
//! Procfile, never both. Readers that only need env call [`load_env`], so a
//! broken Procfile still fails only the deploy.
//!
//! Security: `app` must already be a validated app name; it is joined into
//! paths here. File contents are checked by `app_settings` and
//! `riku_util::parse_settings`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::app_settings::{keys, AppSettings, Source};
use crate::RikuPaths;

const ENV_FILE: &str = "ENV";
const PROCFILE: &str = "Procfile";
const CRON_KIND: &str = "cron";

/// An app's merged env.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppEnv {
    pub env: HashMap<String, String>,
}

fn settings_env(file: &Path, source: Source) -> Result<HashMap<String, String>> {
    match AppSettings::load(file, source) {
        Ok(Some(settings)) => Ok(settings.env()),
        Ok(None) => Ok(HashMap::new()),
        Err(e) => Err(e),
    }
}

fn env_file_map(file: &Path) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    riku_util::parse_settings(file, &mut env)
}

/// The merged env of `app`.
#[allow(clippy::question_mark)]
pub fn load_env(paths: &RikuPaths, app: &str) -> Result<AppEnv> {
    let mut env = match settings_env(&paths.app_settings_repo(app), Source::Repo) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let env_file = match env_file_map(&paths.env_root.join(app).join(ENV_FILE)) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    env.extend(
        env_file
            .into_iter()
            .filter(|(key, _)| !keys::is_table_env_key(key)),
    );

    match AppSettings::load(&paths.app_settings_override(app), Source::Server) {
        Ok(Some(server)) => {
            for key in server.cleared_env() {
                env.remove(&key);
            }
            env.extend(server.env());
        }
        Ok(None) => {}
        Err(e) => return Err(e),
    }
    Ok(AppEnv { env })
}

fn procfile_path(paths: &RikuPaths, app: &str) -> PathBuf {
    paths.app_root.join(app).join(PROCFILE)
}

/// The process list of `app`: repo `riku.toml` sections, else the Procfile,
/// else `None`.
#[allow(clippy::question_mark)]
pub fn load_processes(paths: &RikuPaths, app: &str) -> Result<Option<HashMap<String, String>>> {
    let repo = match AppSettings::load(&paths.app_settings_repo(app), Source::Repo) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };
    let declared = repo.and_then(|settings| settings.processes());
    let procfile = procfile_path(paths, app);
    let from_procfile = match riku_util::parse_procfile(&procfile) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    match (declared, from_procfile) {
        (Some(_), Some(found)) if !found.is_empty() => Err(anyhow!(
            "both riku.toml [processes] and Procfile define processes for '{app}'; keep one"
        )),
        (Some(declared), _) => Ok(Some(declared)),
        (None, found) => Ok(found),
    }
}

/// `(kind, command)` for every `cron*` line of a Procfile, in file order.
/// Unlike [`load_processes`] it keeps repeated kinds: a Procfile may list
/// several `cron:` lines and each one is a job.
fn procfile_cron_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(':'))
        .map(|(kind, command)| (kind.trim().to_string(), command.trim().to_string()))
        .filter(|(kind, _)| kind.starts_with(CRON_KIND))
        .collect()
}

/// The cron jobs of `app` as `(kind, command)`: the repo `riku.toml` `[cron]`
/// entries sorted by kind, else every `cron*` line of the Procfile in order.
#[allow(clippy::question_mark)]
pub fn load_cron_entries(paths: &RikuPaths, app: &str) -> Result<Vec<(String, String)>> {
    let repo = match AppSettings::load(&paths.app_settings_repo(app), Source::Repo) {
        Ok(repo) => repo,
        Err(e) => return Err(e),
    };
    if let Some(declared) = repo.and_then(|settings| settings.processes()) {
        let mut entries: Vec<(String, String)> = declared
            .into_iter()
            .filter(|(kind, _)| kind.starts_with(CRON_KIND))
            .collect();
        entries.sort();
        return Ok(entries);
    }
    let procfile = procfile_path(paths, app);
    match std::fs::read_to_string(&procfile) {
        Ok(text) => Ok(procfile_cron_lines(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(anyhow!("failed to read {}: {e}", procfile.display())),
    }
}

#[cfg(test)]
#[path = "app_config_tests.rs"]
mod tests;

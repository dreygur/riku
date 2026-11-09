//! An app's merged config: the env map and the process list every reader
//! uses, so `riku.toml`, the ENV file and the server override give one
//! answer wherever riku reads them.
//!
//! Env layers, later wins:
//!
//! ```text
//!   <app_root>/<app>/riku.toml      repo file (from the checkout)
//!   <env_root>/<app>/ENV            app env; table keys there are legacy
//!   <env_root>/<app>/riku.toml      server override (riku settings)
//! ```
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

/// An app's merged env.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppEnv {
    pub env: HashMap<String, String>,
    /// Table keys still set in the ENV file; callers that log warn once that
    /// `riku doctor --fix` moves them.
    pub legacy_keys: Vec<String>,
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
    let mut legacy_keys: Vec<String> = env_file
        .keys()
        .filter(|key| keys::is_table_env_key(key))
        .cloned()
        .collect();
    legacy_keys.sort();
    env.extend(env_file);

    match settings_env(&paths.app_settings_override(app), Source::Server) {
        Ok(server) => env.extend(server),
        Err(e) => return Err(e),
    }
    Ok(AppEnv { env, legacy_keys })
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

#[cfg(test)]
#[path = "app_config_tests.rs"]
mod tests;

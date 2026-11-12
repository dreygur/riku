//! Loading an app's merged config for a deploy, with the warnings a deploy
//! prints about it.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::config::app_config::{self, AppEnv};
use crate::config::app_settings::{AppSettings, Source};
use crate::config::RikuPaths;
use crate::util::{deploy_logger::DeployLogger, echo};

fn warn(hint: &str, dlog: &mut DeployLogger) {
    echo(hint, "yellow");
    dlog.log_raw(&format!("[WARN] {hint}"));
}

fn warn_about(loaded: &AppEnv, dlog: &mut DeployLogger) {
    let warnings = crate::util::validate_env_vars(&loaded.env);
    crate::util::print_env_warnings(&warnings);
    for w in &warnings {
        dlog.log_warn(w);
    }
    if !loaded.legacy_keys.is_empty() {
        warn(
            &format!(
                "ENV holds riku settings ({}); run riku doctor --fix to move them into riku settings",
                loaded.legacy_keys.join(", ")
            ),
            dlog,
        );
    }
}

/// The app's merged env from `app_config::load_env` (repo riku.toml, ENV,
/// server override). Invalid values and riku settings in ENV are reported on the
/// terminal and in the deploy log, but do not stop the deploy; an invalid
/// riku.toml or ENV file does.
///
/// Every deploy, container apps included, tightens the env dir to `0700`
/// and the ENV file to `0600`, since a hand written ENV keeps whatever mode
/// it was created with until riku rewrites it.
pub(crate) fn load_deploy_env(
    app: &str,
    paths: &RikuPaths,
    dlog: &mut DeployLogger,
) -> Result<HashMap<String, String>> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = crate::util::create_private_dir(&env_dir) {
        return Err(e);
    }
    let env_file = env_dir.join("ENV");
    if env_file.exists() {
        if let Err(e) = crate::util::restrict_to_owner(&env_file) {
            return Err(e);
        }
    }
    let loaded = match app_config::load_env(paths, app) {
        Ok(loaded) => loaded,
        Err(e) => return Err(e),
    };
    warn_about(&loaded, dlog);
    Ok(loaded.env)
}

/// The merged env as [`load_deploy_env`] reads it, for `riku restart`:
/// nothing is printed and no file mode changes.
pub(crate) fn load_restart_env(app: &str, paths: &RikuPaths) -> Result<HashMap<String, String>> {
    match app_config::load_env(paths, app) {
        Ok(loaded) => Ok(loaded.env),
        Err(e) => Err(e),
    }
}

/// The app's process list for a Procfile deploy.
pub(crate) fn load_deploy_processes(
    app: &str,
    paths: &RikuPaths,
) -> Result<Option<HashMap<String, String>>> {
    match app_config::load_processes(paths, app) {
        Ok(processes) => Ok(processes),
        Err(e) => Err(anyhow!("invalid process list for app '{app}': {e:#}")),
    }
}

/// Container apps run what their compose file or Dockerfile says, so a repo
/// riku.toml that declares processes is refused rather than ignored.
pub(crate) fn refuse_container_processes(app: &str, paths: &RikuPaths) -> Result<()> {
    match AppSettings::load(&paths.app_settings_repo(app), Source::Repo) {
        Ok(Some(settings)) if settings.processes().is_some() => Err(anyhow!(
            "riku.toml [processes] and [cron] are not supported for container apps"
        )),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "deploy_env_tests.rs"]
mod tests;

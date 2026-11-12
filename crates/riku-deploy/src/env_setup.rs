//! Environment variable setup and LIVE_ENV writing for deployed apps.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;

use crate::config::app_settings::keys::is_table_env_key;
use crate::config::RikuPaths;

/// Whether an ENV line sets a riku settings key. riku reads settings only
/// from riku.toml, so such a line must not reach `LIVE_ENV`, where
/// `riku run` would take its `RIKU_MAX_*` limits from it.
fn names_a_setting(line: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(key, _)| is_table_env_key(key.trim()))
}

/// Write `LIVE_ENV` file with resolved environment variables for the app.
///
/// The file contains core riku variables plus everything from the app's
/// `ENV` file, so that the supervisor and workers have a stable snapshot.
/// It holds the same secrets as `ENV`, so it is `0600` too.
#[allow(clippy::question_mark)]
pub fn write_live_env(app: &str, paths: &RikuPaths, env: &HashMap<String, String>) -> Result<()> {
    let live_env_path = paths.env_root.join(app).join("LIVE_ENV");
    let mut content = String::new();
    content.push_str(&format!("APP={}\n", app));
    content.push_str(&format!("LOG_ROOT={}\n", paths.log_root.display()));
    content.push_str(&format!(
        "DATA_ROOT={}\n",
        paths.data_root.join(app).display()
    ));
    if let Ok(home) = std::env::var("HOME") {
        content.push_str(&format!("HOME={}\n", home));
    }
    if let Ok(user) = std::env::var("USER") {
        content.push_str(&format!("USER={}\n", user));
    }

    let env_file = paths.env_root.join(app).join("ENV");
    if env_file.exists() {
        let env_content = match fs::read_to_string(&env_file) {
            Ok(content) => content,
            Err(e) => return Err(anyhow!("cannot read {}: {e}", env_file.display())),
        };
        for line in env_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') && !names_a_setting(line) {
                content.push_str(&format!("{}\n", line));
            }
        }
    }

    // Include any in-memory env vars not yet persisted to ENV file
    for (k, v) in env {
        content.push_str(&format!("{}={}\n", k, v));
    }

    if let Err(e) = crate::util::create_private_dir(&paths.env_root.join(app)) {
        return Err(e);
    }
    if let Err(e) = crate::util::write_private(&live_env_path, content.as_bytes()) {
        return Err(anyhow!("cannot write {}: {e}", live_env_path.display()));
    }
    Ok(())
}

/// Write updated environment variables to the app's ENV file, then trigger a redeploy.
///
/// This is the service-layer entry point for config changes. CLI commands must
/// call this, or `write_env` then `redeploy` when they report the two steps
/// apart, instead of wiring `write_config` + `do_deploy` themselves.
#[allow(clippy::question_mark)]
pub fn update_env_and_redeploy(
    app: &str,
    paths: &crate::config::RikuPaths,
    env: &HashMap<String, String>,
) -> Result<()> {
    if let Err(e) = write_env(app, paths, env) {
        return Err(e);
    }
    redeploy(app, paths)
}

/// Replaces the app's ENV file with `env`, private to the riku user.
#[allow(clippy::question_mark)]
pub fn write_env(
    app: &str,
    paths: &crate::config::RikuPaths,
    env: &HashMap<String, String>,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    if let Err(e) = crate::util::create_private_dir(&env_dir) {
        return Err(e);
    }
    crate::util::write_config(&env_dir.join("ENV"), env, "=")
}

/// Deploys the app again so it runs with its current ENV.
pub fn redeploy(app: &str, paths: &crate::config::RikuPaths) -> Result<()> {
    super::do_deploy(app, paths, &HashMap::new(), None)
}

#[cfg(test)]
#[path = "env_setup_tests.rs"]
mod tests;

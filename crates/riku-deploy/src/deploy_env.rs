//! Loading an app's saved `ENV` for a deploy, with the warnings a deploy
//! prints about it.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::{deploy_logger::DeployLogger, echo};

fn warn_about(env: &HashMap<String, String>, dlog: &mut DeployLogger) {
    let warnings = crate::util::validate_env_vars(env);
    crate::util::print_env_warnings(&warnings);
    for w in &warnings {
        dlog.log_warn(w);
    }
    for (old, new) in crate::util::legacy_env::legacy_keys(env) {
        let hint = format!("ENV key {old} is no longer read, rename it to {new}");
        echo(&hint, "yellow");
        dlog.log_raw(&format!("[WARN] {hint}"));
    }
}

/// The app's `ENV` file as a map, empty when the file does not exist.
/// Invalid and renamed keys are reported on the terminal and in the deploy
/// log, but do not stop the deploy.
pub(crate) fn load_deploy_env(
    app: &str,
    paths: &RikuPaths,
    dlog: &mut DeployLogger,
) -> Result<HashMap<String, String>> {
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env = HashMap::new();
    if env_file.exists() {
        if let Err(e) = crate::util::parse_settings(&env_file, &mut env) {
            return Err(e);
        }
    }
    warn_about(&env, dlog);
    Ok(env)
}

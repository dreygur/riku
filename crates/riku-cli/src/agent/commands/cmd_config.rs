//! Agent commands that read configuration: config:get, config:show.
//!
//! Security model: a readonly agent sees the value of every secret-looking
//! key (`secret_keys::is_secret`), every value that contains one, and every
//! value that references another variable, replaced with `secret_keys::MASK`,
//! so a monitoring key cannot read database URLs or API keys. Staging and
//! production may already change those values, so they see them in full.

use std::collections::{HashMap, HashSet};
use std::fs;

use serde_json::json;

use crate::agent::secret_keys::{is_secret, MASK};
use crate::agent::types::{AgentResponse, AgentScope};
use crate::config::RikuPaths;
use crate::util::{existing_app, parse_settings};

/// The app's ENV, expanded, plus the keys whose raw value references another
/// variable. An unreadable or missing file reads as empty.
fn read_config(paths: &RikuPaths, app: &str) -> (HashMap<String, String>, HashSet<String>) {
    let config_file = paths.env_root.join(app).join("ENV");
    let mut env = HashMap::new();
    let _ = parse_settings(&config_file, &mut env);
    let raw = fs::read_to_string(&config_file).unwrap_or_default();
    let referencing = raw
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(_, value)| value.contains('$'))
        .map(|(key, _)| key.trim().to_string())
        .collect();
    (env, referencing)
}

/// The config as `scope` may see it. ENV values are expanded, so a plain key
/// such as `COPY=$DATABASE_URL` can carry a secret, even one redefined later
/// in the file. For a readonly scope every value that references a variable
/// is masked, and so is any value that contains a secret key's value.
fn visible_config(
    scope: &AgentScope,
    env: HashMap<String, String>,
    referencing: &HashSet<String>,
) -> HashMap<String, String> {
    if *scope != AgentScope::Readonly {
        return env;
    }
    let secrets: Vec<String> = env
        .iter()
        .filter(|(key, value)| is_secret(key) && !value.is_empty())
        .map(|(_, value)| value.clone())
        .collect();
    env.into_iter()
        .map(|(key, value)| {
            let hidden = is_secret(&key)
                || referencing.contains(&key)
                || secrets.iter().any(|secret| value.contains(secret));
            match hidden {
                true => (key, MASK.to_string()),
                false => (key, value),
            }
        })
        .collect()
}

pub fn cmd_agent_config_get(
    paths: &RikuPaths,
    scope: &AgentScope,
    app: &str,
    key: &str,
) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let (env, referencing) = read_config(paths, &app);
    if let Some(value) = visible_config(scope, env, &referencing).remove(key) {
        return AgentResponse::success(json!({"key": key, "value": value}));
    }

    AgentResponse::error("KEY_NOT_FOUND", &format!("Key '{}' not found", key))
}

pub fn cmd_agent_config_show(paths: &RikuPaths, scope: &AgentScope, app: &str) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let (env, referencing) = read_config(paths, &app);
    let config = visible_config(scope, env, &referencing);

    AgentResponse::success(json!({"app": app, "config": config}))
}

#[cfg(test)]
#[path = "cmd_config_tests.rs"]
mod tests;

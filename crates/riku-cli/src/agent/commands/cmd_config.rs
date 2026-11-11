//! Agent commands that read configuration: config:get, config:show.
//!
//! Security model: a readonly agent sees the value of every secret-looking
//! key (`secret_keys::is_secret`), and every value that contains one,
//! replaced with `secret_keys::MASK`, so a monitoring key cannot read
//! database URLs or API keys. Staging and production may already change
//! those values, so they see them in full.

use std::collections::HashMap;

use serde_json::json;

use crate::agent::secret_keys::{is_secret, MASK};
use crate::agent::types::{AgentResponse, AgentScope};
use crate::config::RikuPaths;
use crate::util::existing_app;

/// The config as `scope` may see it. ENV values are expanded, so a plain
/// key such as `COPY=$DATABASE_URL` carries a secret too: for a readonly
/// scope any value containing a secret key's value is masked as well.
fn visible_config(scope: &AgentScope, env: HashMap<String, String>) -> HashMap<String, String> {
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
            let hidden = is_secret(&key) || secrets.iter().any(|secret| value.contains(secret));
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

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        if let Ok(settings) = crate::util::parse_settings(&config_file, &mut env) {
            if let Some(value) = visible_config(scope, settings).remove(key) {
                return AgentResponse::success(json!({"key": key, "value": value}));
            }
        }
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

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut config = HashMap::new();

    if config_file.exists() {
        let _ = crate::util::parse_settings(&config_file, &mut config);
    }
    let config = visible_config(scope, config);

    AgentResponse::success(json!({"app": app, "config": config}))
}

#[cfg(test)]
#[path = "cmd_config_tests.rs"]
mod tests;

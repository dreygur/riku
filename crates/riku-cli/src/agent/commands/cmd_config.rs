//! Agent commands that read configuration: config:get, config:show.
//!
//! Security model: a readonly agent sees the value of every secret-looking
//! key (`secret_keys::is_secret`) replaced with `secret_keys::MASK`, so a
//! monitoring key cannot read database URLs or API keys. Staging and
//! production may already change those values, so they see them in full.

use std::collections::HashMap;

use serde_json::json;

use crate::agent::secret_keys::{is_secret, MASK};
use crate::agent::types::{AgentResponse, AgentScope};
use crate::config::RikuPaths;
use crate::util::existing_app;

fn visible(scope: &AgentScope, key: &str, value: String) -> String {
    match scope {
        AgentScope::Readonly if is_secret(key) => MASK.to_string(),
        _ => value,
    }
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
        if let Ok(mut settings) = crate::util::parse_settings(&config_file, &mut env) {
            if let Some(value) = settings.remove(key) {
                let value = visible(scope, key, value);
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
    let config: HashMap<String, String> = config
        .into_iter()
        .map(|(key, value)| {
            let value = visible(scope, &key, value);
            (key, value)
        })
        .collect();

    AgentResponse::success(json!({"app": app, "config": config}))
}

#[cfg(test)]
#[path = "cmd_config_tests.rs"]
mod tests;

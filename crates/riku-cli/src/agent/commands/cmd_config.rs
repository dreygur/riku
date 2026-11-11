/// Agent commands that read configuration: config:get, config:show.
use std::collections::HashMap;

use serde_json::json;

use crate::config::RikuPaths;
use crate::util::existing_app;

use crate::agent::types::AgentResponse;

pub fn cmd_agent_config_get(paths: &RikuPaths, app: &str, key: &str) -> AgentResponse {
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
            if let Some(val) = settings.get(key) {
                return AgentResponse::success(json!({"key": key, "value": val}));
            }
        }
    }

    AgentResponse::error("KEY_NOT_FOUND", &format!("Key '{}' not found", key))
}

pub fn cmd_agent_config_show(paths: &RikuPaths, app: &str) -> AgentResponse {
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

    AgentResponse::success(json!({"app": app, "config": config}))
}

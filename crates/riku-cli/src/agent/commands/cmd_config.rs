/// Agent commands for configuration: config:get, config:set, config:show.
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::config::RikuPaths;
use crate::util::exit_if_invalid;

use crate::agent::types::AgentResponse;

pub fn cmd_agent_config_get(paths: &RikuPaths, app: &str, key: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
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

pub fn cmd_agent_config_set(_paths: &RikuPaths, app: &str, settings: &[String]) -> AgentResponse {
    let critical_keys = [
        "DATABASE_URL",
        "SECRET_KEY",
        "PASSWORD",
        "API_KEY",
        "PRIVATE_KEY",
    ];

    for setting in settings {
        if let Some(key) = setting.split('=').next() {
            if critical_keys.contains(&key.to_uppercase().as_str()) {
                let token = format!(
                    "config-{}-{}",
                    app,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                return AgentResponse::confirmation_required("config:set", app, &token);
            }
        }
    }

    AgentResponse::success(json!({
        "message": format!("Configuration update initiated for {}", app),
        "settings_count": settings.len(),
        "note": "Use 'riku config:set' for immediate effect"
    }))
}

pub fn cmd_agent_config_show(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
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

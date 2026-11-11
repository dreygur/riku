//! `agent config:set` and `config:unset`.
//!
//! Security model: the scope check (staging and up) happens in `execute.rs`.
//! A change that touches a secret-looking key needs a confirm token, bound to
//! the exact request (`confirm::bound_action`). Which keys a request changes
//! is read from the parsed result the config service will write, not from
//! the agent's words. Validation (riku settings, malformed pairs, newlines)
//! is the same service `riku config set` uses, and runs before a token is
//! issued or anything is written.

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;

use crate::agent::confirm;
use crate::agent::secret_keys::is_secret;
use crate::agent::types::AgentResponse;
use crate::apps::{config_after_set, config_after_unset};
use crate::config::RikuPaths;
use crate::deploy::env_setup::{redeploy, write_env};
use crate::util::{existing_app, parse_settings};

/// What the agent asked for, which decides the confirmation action name.
#[derive(Clone, Copy)]
enum ConfigChange {
    Set,
    Unset,
}

impl ConfigChange {
    fn action(self) -> &'static str {
        match self {
            ConfigChange::Set => "config:set",
            ConfigChange::Unset => "config:unset",
        }
    }
}

/// Keys whose value differs between `before` and `after`, added and removed
/// ones included. The secret check runs on these, not on the agent's words,
/// so it sees exactly what the config service parsed and will write.
fn changed_keys(before: &HashMap<String, String>, after: &HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<String> = after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(*value))
        .map(|(key, _)| key.clone())
        .chain(
            before
                .keys()
                .filter(|key| !after.contains_key(*key))
                .cloned(),
        )
        .collect();
    keys.sort();
    keys
}

fn apply(
    paths: &RikuPaths,
    app: &str,
    env: &HashMap<String, String>,
    keys: &[String],
) -> AgentResponse {
    if let Err(e) = write_env(app, paths, env) {
        return AgentResponse::error("CONFIG_FAILED", &format!("config not saved: {e}"));
    }
    match redeploy(app, paths) {
        Ok(()) => AgentResponse::success(json!({"app": app, "keys": keys, "redeployed": true})),
        Err(e) => {
            let message = format!("config saved, but the redeploy failed: {e}");
            AgentResponse::error("DEPLOY_FAILED", &message)
        }
    }
}

fn change_config(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    change: ConfigChange,
    words: &[String],
    confirm_token: Option<&str>,
) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(_) => {
            let message = format!("Application '{app}' not found");
            return AgentResponse::error("APP_NOT_FOUND", &message);
        }
    };
    if words.is_empty() {
        return AgentResponse::error("INVALID_PARAMETERS", "Missing keys");
    }
    let mut before = HashMap::new();
    if let Err(e) = parse_settings(&paths.env_root.join(&app).join("ENV"), &mut before) {
        return AgentResponse::error("CONFIG_FAILED", &e.to_string());
    }
    let planned = match change {
        ConfigChange::Set => config_after_set(paths, &app, words),
        ConfigChange::Unset => config_after_unset(paths, &app, words),
    };
    let after = match planned {
        Ok(env) => env,
        Err(e) => return AgentResponse::error("INVALID_PARAMETERS", &e.to_string()),
    };
    let keys = changed_keys(&before, &after);
    if !keys.iter().any(|key| is_secret(key)) {
        return apply(paths, &app, &after, &keys);
    }
    let bound = confirm::bound_action(change.action(), words);
    if let Some(stop) = confirm::gate(state, change.action(), &bound, &app, confirm_token) {
        return stop;
    }
    apply(paths, &app, &after, &keys)
}

/// Sets `KEY=VALUE` pairs and redeploys.
pub fn cmd_agent_config_set(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    settings: &[String],
    confirm_token: Option<&str>,
) -> AgentResponse {
    change_config(
        paths,
        state,
        app,
        ConfigChange::Set,
        settings,
        confirm_token,
    )
}

/// Removes keys and redeploys.
pub fn cmd_agent_config_unset(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    keys: &[String],
    confirm_token: Option<&str>,
) -> AgentResponse {
    change_config(paths, state, app, ConfigChange::Unset, keys, confirm_token)
}

#[cfg(test)]
#[path = "cmd_config_write_tests.rs"]
mod tests;

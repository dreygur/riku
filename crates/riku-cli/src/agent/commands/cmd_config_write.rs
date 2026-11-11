//! `agent config:set` and `config:unset`.
//!
//! Security model: the scope check (staging and up) happens in `execute.rs`.
//! A change that touches a secret-looking key needs a human confirm token.
//! The token is bound to a SHA-256 of the exact request, so a token approved
//! for one value cannot be spent on another. Validation (riku settings,
//! malformed pairs, newlines) is the same service `riku config set` uses, and
//! runs before anything is written.

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::agent::confirm;
use crate::agent::types::AgentResponse;
use crate::apps::{config_after_set, config_after_unset};
use crate::config::RikuPaths;
use crate::deploy::env_setup::{redeploy, write_env};
use crate::util::{existing_app, parse_settings};

const SECRET_MARKERS: [&str; 5] = ["SECRET", "PASSWORD", "TOKEN", "_KEY", "DATABASE_URL"];

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

fn is_secret(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|marker| key.contains(marker))
}

/// The confirm record's action line: the command plus a digest of the exact
/// words, so the line never holds a secret or a newline.
fn bound_action(change: ConfigChange, words: &[String]) -> String {
    let mut digest = Sha256::new();
    for word in words {
        digest.update(word.as_bytes());
        digest.update([0]);
    }
    format!("{}:{}", change.action(), hex::encode(digest.finalize()))
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
    let action = bound_action(change, words);
    let token = match confirm_token {
        Some(token) => token,
        None => {
            return match confirm::issue(state, &action, &app) {
                Ok(token) => AgentResponse::confirmation_required(change.action(), &app, &token),
                Err(e) => {
                    let message = format!("cannot issue a confirmation token: {e}");
                    AgentResponse::error("STATE_UNAVAILABLE", &message)
                }
            };
        }
    };
    if let Some(refusal) = confirm::refused(state, &action, &app, token) {
        return refusal;
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

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
use crate::util::existing_app;

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

fn key_of(word: &str) -> &str {
    word.split_once('=').map_or(word, |(key, _)| key)
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

fn apply(paths: &RikuPaths, app: &str, change: ConfigChange, words: &[String]) -> AgentResponse {
    let planned = match change {
        ConfigChange::Set => config_after_set(paths, app, words),
        ConfigChange::Unset => config_after_unset(paths, app, words),
    };
    let env: HashMap<String, String> = match planned {
        Ok(env) => env,
        Err(e) => return AgentResponse::error("INVALID_PARAMETERS", &e.to_string()),
    };
    if let Err(e) = write_env(app, paths, &env) {
        return AgentResponse::error("CONFIG_FAILED", &format!("config not saved: {e}"));
    }
    let keys: Vec<&str> = words.iter().map(|word| key_of(word)).collect();
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
    if !words.iter().any(|word| is_secret(key_of(word))) {
        return apply(paths, &app, change, words);
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
    apply(paths, &app, change, words)
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

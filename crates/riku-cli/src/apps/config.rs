// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::app_settings::keys;
use crate::config::RikuPaths;
use crate::util::{display, parse_settings, require_app};

/// Show app configuration (ENV file).
pub fn cmd_config_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let content = match fs::read_to_string(&config_file) {
            Ok(content) => content,
            Err(e) => return Err(e.into()),
        };
        println!("{}", content.trim());
    } else {
        display::warn(&format!("app '{}' not deployed, no config found.", app));
    }
    Ok(())
}

/// Get a single config value.
pub fn cmd_config_get(paths: &RikuPaths, app: &str, key: &str) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        let settings = match parse_settings(&config_file, &mut env) {
            Ok(settings) => settings,
            Err(e) => return Err(e),
        };
        // Raw, for `$(riku config get app KEY)` in scripts.
        if let Some(val) = settings.get(key) {
            println!("{val}");
        }
    } else {
        display::warn(&format!("no active configuration for '{}'", app));
    }
    Ok(())
}

/// Write `env` as the app's ENV, print `◇  <label>  <keys>`, then redeploy.
/// Only key names are printed: values are often secrets, and this output
/// reaches terminals, CI logs and agent transcripts.
fn write_and_redeploy(
    paths: &RikuPaths,
    app: &str,
    env: &HashMap<String, String>,
    label: &str,
    keys: &[&str],
) -> Result<()> {
    if let Err(e) = crate::deploy::env_setup::write_env(app, paths, env) {
        return Err(e);
    }
    display::step_done(label, &keys.join(", "));
    crate::deploy::env_setup::redeploy(app, paths)
}

/// The key of each `KEY=VALUE` argument, in order.
fn set_keys(settings: &[String]) -> Vec<&str> {
    settings
        .iter()
        .filter_map(|s| s.split_once('=').map(|(key, _)| key.trim()))
        .collect()
}

/// Set config values (KEY=VALUE pairs), write config, trigger deploy.
pub fn cmd_config_set(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let env = match config_after_set(paths, &app, settings) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    write_and_redeploy(paths, &app, &env, "Set", &set_keys(settings))
}

/// The app's ENV with `settings` applied, without writing it or printing
/// anything (the agent path calls it before asking for confirmation). Each
/// argument is one `KEY=VALUE` pair taken as is, so a value may hold spaces;
/// the caller's shell or the ssh split already removed any quoting. Refuses
/// riku settings, malformed pairs and newlines. `app` must already be
/// validated.
pub fn config_after_set(
    paths: &RikuPaths,
    app: &str,
    settings: &[String],
) -> Result<HashMap<String, String>> {
    let config_file = paths.env_root.join(app).join("ENV");
    let mut env = HashMap::new();
    if let Err(e) = parse_settings(&config_file, &mut env) {
        return Err(e);
    }

    for s in settings {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v = s[eq_pos + 1..].trim().to_string();

            if let Some(path) = keys::path_for_env(&k) {
                anyhow::bail!("{k} is a riku setting; use riku settings set {app} {path}=...");
            }

            if k.contains(['\n', '\r']) || v.contains(['\n', '\r']) {
                anyhow::bail!(
                    "setting '{}' contains a newline, which would corrupt the ENV file",
                    k.trim()
                );
            }

            env.insert(k, v);
        } else {
            anyhow::bail!("malformed setting '{}': expected KEY=VALUE", s);
        }
    }
    Ok(env)
}

/// Unset config values, write config, trigger deploy.
pub fn cmd_config_unset(paths: &RikuPaths, app: &str, keys: &[String]) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let env = match config_after_unset(paths, &app, keys) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let names: Vec<&str> = keys.iter().map(String::as_str).collect();
    write_and_redeploy(paths, &app, &env, "Unset", &names)
}

/// The app's ENV without `keys`, without writing it or printing anything.
/// `app` must already be validated.
pub fn config_after_unset(
    paths: &RikuPaths,
    app: &str,
    keys: &[String],
) -> Result<HashMap<String, String>> {
    let config_file = paths.env_root.join(app).join("ENV");
    let mut env = HashMap::new();
    if let Err(e) = parse_settings(&config_file, &mut env) {
        return Err(e);
    }

    for key in keys {
        env.remove(key);
    }
    Ok(env)
}

/// Show live running configuration (LIVE_ENV file).
pub fn cmd_config_live(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };

    let live_config = paths.env_root.join(&app).join("LIVE_ENV");
    if live_config.exists() {
        let content = match fs::read_to_string(&live_config) {
            Ok(content) => content,
            Err(e) => return Err(e.into()),
        };
        println!("{}", content.trim());
    } else {
        display::warn(&format!("app '{}' not deployed, no config found.", app));
    }
    Ok(())
}

/// Simple shell-like splitting of a string on whitespace, respecting quotes.
pub(super) fn shell_split(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for c in input.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }
        if c == '\\' && !in_single_quote {
            escape_next = true;
            continue;
        }
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }
        if c.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

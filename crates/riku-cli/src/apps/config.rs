use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid, parse_settings};

/// Show app configuration (ENV file).
pub fn cmd_config_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
        display::note(content.trim());
    } else {
        display::warn(&format!(
            "Warning: app '{}' not deployed, no config found.",
            app
        ));
    }
    Ok(())
}

/// Get a single config value.
pub fn cmd_config_get(paths: &RikuPaths, app: &str, key: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        let settings = parse_settings(&config_file, &mut env)?;
        if let Some(val) = settings.get(key) {
            display::note(val);
        }
    } else {
        display::note(&format!("Warning: no active configuration for '{}'", app));
    }
    Ok(())
}

/// Set config values (KEY=VALUE pairs), write config, trigger deploy.
pub fn cmd_config_set(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    // Join all settings and split them shell-style
    let joined = settings.join(" ");
    let parts = shell_split(&joined);

    for s in &parts {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v = s[eq_pos + 1..].trim().to_string();
            display::note(&format!("Setting {}={} for '{}'", k, v, app));
            env.insert(k, v);
        } else {
            display::error(&format!("Error: malformed setting '{}'", s));
            return Ok(());
        }
    }
    crate::deploy::env_setup::update_env_and_redeploy(&app, paths, &env)?;
    Ok(())
}

/// Unset config values, write config, trigger deploy.
pub fn cmd_config_unset(paths: &RikuPaths, app: &str, keys: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    for s in keys {
        if env.remove(s).is_some() {
            display::note(&format!("Unsetting {} for '{}'", s, app));
        }
    }
    crate::deploy::env_setup::update_env_and_redeploy(&app, paths, &env)?;
    Ok(())
}

/// Show live running configuration (LIVE_ENV file).
pub fn cmd_config_live(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let live_config = paths.env_root.join(&app).join("LIVE_ENV");
    if live_config.exists() {
        let content = fs::read_to_string(&live_config)?;
        display::note(content.trim());
    } else {
        display::warn(&format!(
            "Warning: app '{}' not deployed, no config found.",
            app
        ));
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

//! `riku settings`: the server side riku settings of an app, kept in
//! `<env_root>/<app>/riku.toml` (the server override).
//!
//! Security: the app name is validated and must name an existing app before
//! it becomes part of a path. The file is checked as a server override
//! before it is written, written `0600` from creation in a `0700` directory,
//! and `show` masks secret keys.

#![allow(clippy::question_mark)]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};

use crate::config::app_settings::{keys, AppSettings, Source};
use crate::config::RikuPaths;
use crate::util::{create_private_dir, display, validate_app_name, write_private};

use super::config::shell_split;

const MASK: &str = "***";

/// `app` validated and known, or why not. Unlike `exit_if_invalid` it
/// returns instead of exiting, so callers and tests see the error.
fn known_app(paths: &RikuPaths, app: &str) -> Result<String> {
    let app = match validate_app_name(app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if !paths.app_root.join(&app).is_dir() {
        bail!("app '{app}' not found");
    }
    Ok(app)
}

fn load_override(paths: &RikuPaths, app: &str) -> Result<AppSettings> {
    match AppSettings::load(&paths.app_settings_override(app), Source::Server) {
        Ok(settings) => Ok(settings.unwrap_or_default()),
        Err(e) => Err(e),
    }
}

#[allow(clippy::question_mark)]
fn save_override(paths: &RikuPaths, app: &str, settings: &AppSettings) -> Result<()> {
    let file = paths.app_settings_override(app);
    let text = match settings.to_toml() {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    if let Err(e) = AppSettings::parse(&text, &file, Source::Server) {
        return Err(e);
    }
    if let Err(e) = create_private_dir(&paths.env_root.join(app)) {
        return Err(e);
    }
    write_private(&file, text.as_bytes())
}

/// `path = value` lines for `show`, secret values masked.
fn show_lines(settings: &AppSettings) -> Vec<String> {
    settings
        .entries()
        .into_iter()
        .map(|(path, value, secret)| {
            let shown = if secret { MASK.to_string() } else { value };
            format!("{path} = {shown}")
        })
        .collect()
}

/// Apply `key=value` pairs to the override and write it.
#[allow(clippy::question_mark)]
fn apply_set(paths: &RikuPaths, app: &str, pairs: &[String]) -> Result<()> {
    let mut settings = match load_override(paths, app) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    for pair in shell_split(&pairs.join(" ")) {
        let Some((key, value)) = pair.split_once('=') else {
            bail!("malformed setting '{pair}': expected key=value");
        };
        if let Err(e) = settings.set(key.trim(), value.trim()) {
            return Err(e);
        }
    }
    save_override(paths, app, &settings)
}

/// Remove `keys` from the override and write it.
#[allow(clippy::question_mark)]
fn apply_unset(paths: &RikuPaths, app: &str, keys_to_remove: &[String]) -> Result<()> {
    let mut settings = match load_override(paths, app) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    for key in keys_to_remove {
        if keys::find(key).is_none() {
            bail!("unknown key {key}");
        }
        settings.unset(key);
    }
    save_override(paths, app, &settings)
}

fn redeploy(paths: &RikuPaths, app: &str) -> Result<()> {
    display::note(&format!("Redeploying '{app}'"));
    crate::deploy::do_deploy(app, paths, &HashMap::new(), None)
}

/// Print the server settings of `app`.
#[allow(clippy::question_mark)]
pub fn cmd_settings_show(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let settings = match load_override(paths, &app) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    let lines = show_lines(&settings);
    if lines.is_empty() {
        display::note(&format!("No server settings for '{app}'."));
    }
    for line in lines {
        display::note(&line);
    }
    Ok(())
}

/// Print one server setting of `app`; nothing when it is not set.
#[allow(clippy::question_mark)]
pub fn cmd_settings_get(paths: &RikuPaths, app: &str, key: &str) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if keys::find(key).is_none() {
        return Err(anyhow!("unknown key {key}"));
    }
    let settings = match load_override(paths, &app) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    if let Some(value) = settings.get(key) {
        display::note(&value);
    }
    Ok(())
}

/// Set server settings of `app`, then redeploy it.
#[allow(clippy::question_mark)]
pub fn cmd_settings_set(paths: &RikuPaths, app: &str, pairs: &[String]) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if let Err(e) = apply_set(paths, &app, pairs) {
        return Err(e);
    }
    redeploy(paths, &app)
}

/// Remove server settings of `app`, then redeploy it.
#[allow(clippy::question_mark)]
pub fn cmd_settings_unset(paths: &RikuPaths, app: &str, keys_to_remove: &[String]) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if let Err(e) = apply_unset(paths, &app, keys_to_remove) {
        return Err(e);
    }
    redeploy(paths, &app)
}

#[cfg(test)]
#[path = "settings_cmd_tests.rs"]
mod tests;

//! Moving one app's riku keys out of its ENV file into its server override
//! (`<env_root>/<app>/riku.toml`), for `riku doctor --fix`.
//!
//! A key moves only when the override renders it back to the exact value it
//! had in ENV, so the app's merged config does not change. Keys that would
//! change (a `false` flag the reader treats as on, a value of the wrong
//! type) stay in ENV with a warning. Order: `ENV.bak` once, then the checked
//! override, then ENV without the moved keys; every other ENV line is kept
//! byte for byte.
//!
//! Security: ENV holds secrets, so every file written keeps the mode of the
//! file it replaces (`0600` when new) and is replaced by rename.

#![allow(clippy::question_mark)]

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::app_settings::{keys, AppSettings, Source};
use crate::config::RikuPaths;

use super::migrate_check;
use super::mode_write::{back_up_once, replace_keeping_mode};
use super::Check;

const NAME: &str = "settings migration";

/// The table path an env key moves to.
fn path_of(env_key: &str, settings: &AppSettings) -> Option<String> {
    if let Some(path) = keys::path_for_env(env_key) {
        return Some(path);
    }
    match settings.get("runtime.name") {
        Some(runtime) if keys::runtime_version_env(&runtime) == env_key => {
            Some("runtime.version".to_string())
        }
        _ => None,
    }
}

/// Env keys whose value would change if moved, removed from `candidate`.
fn drop_lossy(
    candidate: &mut AppSettings,
    moved: &[String],
    env: &HashMap<String, String>,
) -> Result<Vec<String>> {
    let text = match candidate.to_toml() {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    let rendered = match AppSettings::parse(&text, Path::new("riku.toml"), Source::Server) {
        Ok(settings) => settings.env(),
        Err(e) => return Err(e),
    };
    let lossy: Vec<String> = moved
        .iter()
        .filter(|key| rendered.get(*key) != env.get(*key))
        .cloned()
        .collect();
    for key in &lossy {
        if let Some(path) = path_of(key, candidate) {
            candidate.unset(&path);
        }
    }
    Ok(lossy)
}

/// `existing` override values on top of `candidate` (existing wins).
fn merged(mut candidate: AppSettings, existing: &AppSettings) -> Result<AppSettings> {
    for (path, value, _) in existing.entries() {
        if let Err(e) = candidate.set(&path, &value) {
            return Err(e);
        }
    }
    Ok(candidate)
}

fn env_line_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

fn without_keys(text: &str, moved: &BTreeSet<String>) -> String {
    text.split_inclusive('\n')
        .filter(|line| !env_line_key(line).is_some_and(|key| moved.contains(key)))
        .collect()
}

fn write_override(paths: &RikuPaths, app: &str, text: &str) -> Result<()> {
    let file = paths.app_settings_override(app);
    if let Err(e) = AppSettings::parse(text, &file, Source::Server) {
        return Err(e);
    }
    replace_keeping_mode(&file, text)
}

fn migrate_app_or_err(paths: &RikuPaths, app: &str) -> Result<Vec<Check>> {
    let env_path = paths.env_root.join(app).join("ENV");
    let raw = match fs::read_to_string(&env_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", env_path.display())),
    };
    let mut env = HashMap::new();
    let env = match crate::util::parse_settings(&env_path, &mut env) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let (mut candidate, moved) = AppSettings::from_env(&env);
    let lossy = match drop_lossy(&mut candidate, &moved, &env) {
        Ok(lossy) => lossy,
        Err(e) => return Err(e),
    };
    let mut unfit: Vec<&String> = env
        .keys()
        .filter(|key| keys::is_table_env_key(key) && !moved.contains(*key))
        .collect();
    unfit.sort();
    let mut checks: Vec<Check> = lossy
        .iter()
        .map(|key| {
            Check::warn(
                NAME,
                format!("{app}: {key} left in ENV, its value would change in riku settings"),
            )
        })
        .chain(unfit.iter().map(|key| {
            Check::warn(
                NAME,
                format!("{app}: {key} left in ENV, its value does not fit the riku setting"),
            )
        }))
        .collect();
    let to_move: BTreeSet<String> = moved
        .into_iter()
        .filter(|key| !lossy.contains(key))
        .collect();
    if to_move.is_empty() {
        return Ok(checks);
    }

    let existing = match AppSettings::load(&paths.app_settings_override(app), Source::Server) {
        Ok(existing) => existing.unwrap_or_default(),
        Err(e) => return Err(e),
    };
    let settings = match merged(candidate, &existing) {
        Ok(settings) => settings,
        Err(e) => return Err(e),
    };
    let override_text = match settings.to_toml() {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    let env_text = without_keys(&raw, &to_move);
    let changed = match migrate_check::changed_keys(paths, app, &env_text, &override_text) {
        Ok(changed) => changed,
        Err(e) => return Err(e),
    };
    if !changed.is_empty() {
        checks.push(Check::warn(
            NAME,
            format!(
                "{app}: nothing moved, it would change {}; move those keys by hand",
                changed.join(", ")
            ),
        ));
        return Ok(checks);
    }
    if let Err(e) = back_up_once(&env_path, &raw) {
        return Err(e);
    }
    if let Err(e) = write_override(paths, app, &override_text) {
        return Err(e);
    }
    if let Err(e) = replace_keeping_mode(&env_path, &env_text) {
        return Err(e);
    }
    let list: Vec<&str> = to_move.iter().map(String::as_str).collect();
    checks.push(Check::ok(
        NAME,
        format!("{app}: moved {} into riku settings", list.join(", ")),
    ));
    Ok(checks)
}

/// Checks for one app: what moved, what stayed and why, or the error. Holds
/// the app's deploy lock, so a push cannot rewrite ENV at the same time.
pub fn migrate_app(paths: &RikuPaths, app: &str) -> Vec<Check> {
    let _lock = match crate::deploy::lock::acquire_for_deploy(app, paths) {
        Ok(lock) => lock,
        Err(e) => return vec![Check::fail(NAME, format!("{app}: {e:#}"))],
    };
    match migrate_app_or_err(paths, app) {
        Ok(checks) => checks,
        Err(e) => vec![Check::fail(NAME, format!("{app}: {e:#}"))],
    }
}

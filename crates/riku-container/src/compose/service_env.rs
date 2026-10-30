//! A service's `environment` and `env_file`.
//!
//! The `env_file` paths come from a pushed compose file, so each must resolve
//! inside the app directory (lexically first, then through symlinks when read)
//! and is read with the compose `.env` parser. `environment` wins over
//! `env_file`. Every final value is checked for line breaks and NUL, so no
//! value can split into extra engine arguments. Messages name keys and files,
//! never values.

use super::environment::{is_valid_key, parse_dotenv, read_app_file};
use super::paths::resolve_inside;
use super::scalar::{check_text, text_value};
use super::{Mapping, ServiceContext, Value};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// Merge `env_file` entries with `environment` for one service.
pub(super) fn parse_environment(
    ctx: &ServiceContext,
    service: &Mapping,
) -> Result<BTreeMap<String, String>> {
    let mut merged = match service.get("env_file") {
        Some(files) => match read_env_files(ctx, files) {
            Ok(vars) => vars,
            Err(e) => return Err(e),
        },
        None => BTreeMap::new(),
    };
    if let Some(inline) = service.get("environment") {
        match inline_environment(ctx, inline) {
            Ok(vars) => merged.extend(vars),
            Err(e) => return Err(e),
        }
    }
    for (key, value) in &merged {
        if let Err(e) = check_text(value, &format!("environment '{key}'")) {
            return Err(e);
        }
    }
    Ok(merged)
}

fn read_env_files(ctx: &ServiceContext, value: &Value) -> Result<BTreeMap<String, String>> {
    let names: Vec<&str> = match value {
        Value::String(name) => vec![name.as_str()],
        Value::Array(items) => {
            let mut names = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(name) => names.push(name.as_str()),
                    _ => bail!("env_file entries must be strings"),
                }
            }
            names
        }
        _ => bail!("env_file must be a string or a list of strings"),
    };
    let mut vars = BTreeMap::new();
    for name in names {
        let path = match resolve_inside(ctx.app_path, ctx.compose_dir, name, "env_file") {
            Ok(path) => path,
            Err(e) => return Err(e),
        };
        let text = match read_app_file(ctx.app_path, &path) {
            Ok(text) => text,
            Err(e) => return Err(e),
        };
        // A bare KEY and `${VAR}` in an env_file read the project variables
        // (the `.env` overlaid by the app ENV). The service parse has no
        // warning channel, so unset variables expand to "" without one.
        match parse_dotenv(name, &text, ctx.vars, &mut Vec::new()) {
            Ok(parsed) => vars.extend(parsed),
            Err(e) => return Err(e),
        }
    }
    Ok(vars)
}

fn inline_environment(ctx: &ServiceContext, value: &Value) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();
    match value {
        Value::Object(entries) => {
            for (key, raw) in entries {
                if let Err(e) = check_key(key) {
                    return Err(e);
                }
                if raw.is_null() {
                    continue;
                }
                match text_value(raw, &format!("environment '{key}'")) {
                    Ok(text) => vars.insert(key.clone(), text),
                    Err(e) => return Err(e),
                };
            }
        }
        Value::Array(items) => {
            for item in items {
                let Value::String(entry) = item else {
                    bail!("environment list entries must be strings like KEY=value");
                };
                let (key, text) = match entry.split_once('=') {
                    Some((key, text)) => (key, Some(text.to_string())),
                    None => (entry.as_str(), ctx.vars.get(entry.as_str()).cloned()),
                };
                if let Err(e) = check_key(key) {
                    return Err(e);
                }
                if let Some(text) = text {
                    vars.insert(key.to_string(), text);
                }
            }
        }
        _ => bail!("environment must be a mapping or a list of KEY=value"),
    }
    Ok(vars)
}

fn check_key(key: &str) -> Result<()> {
    if is_valid_key(key) {
        return Ok(());
    }
    bail!("invalid environment variable name {key:?}")
}

#[cfg(test)]
#[path = "service_env_tests.rs"]
mod tests;

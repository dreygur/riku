//! Reading a compose file into a checked, interpolated YAML tree.
//!
//! # Security model
//!
//! The compose file and the `.env` next to it come from a git push. Both must
//! resolve inside the app directory and are size capped. The YAML parser
//! rejects duplicate keys and bounds alias expansion, merge keys are applied
//! by the parser, and the key check runs on that merged result. Interpolation
//! touches string values only, never keys, and runs once, so a variable value
//! is never re-expanded.

use super::environment::{read_app_file, read_dotenv};
use super::interpolate::interpolate;
use super::keys::check_keys;
use super::{ComposeFile, Mapping, Value};
use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// A compose file after merge keys, key checks and interpolation. Service
/// fields are still raw YAML; later stages turn them into typed values.
#[derive(Debug)]
pub struct RawProject {
    /// Service definitions by service name.
    pub services: BTreeMap<String, Value>,
    /// Top level named volume declarations by volume name. A volume declared
    /// with no settings is `Value::Null`.
    pub volumes: BTreeMap<String, Value>,
    /// The top level `name` key. It sets the project name docker compose used
    /// for existing volumes, so it is kept for volume naming.
    pub name: Option<String>,
    /// Non fatal findings, such as unset variables.
    pub warnings: Vec<String>,
}

/// Read, check and interpolate the compose file.
///
/// Variables come from the `.env` next to the compose file, overlaid by
/// `app_env` (the app's own ENV).
pub fn read_raw(
    app_path: &Path,
    compose: &ComposeFile,
    app_env: &BTreeMap<String, String>,
) -> Result<RawProject> {
    let compose_path = app_path.join(compose.relative());
    let text = match read_app_file(app_path, &compose_path) {
        Ok(text) => text,
        Err(e) => return Err(e),
    };
    let mut document: Value = match serde_saphyr::from_str(&text) {
        Ok(document) => document,
        Err(e) => return Err(anyhow!("{}: {e}", compose.relative().display())),
    };
    let Value::Object(project) = &document else {
        bail!(
            "{}: the top level must be a mapping",
            compose.relative().display()
        );
    };
    if let Err(e) = check_keys(project) {
        return Err(e);
    }

    let dotenv_dir = compose_path.parent().unwrap_or(app_path);
    let mut vars = match read_dotenv(app_path, &dotenv_dir.join(".env")) {
        Ok(vars) => vars,
        Err(e) => return Err(e),
    };
    vars.extend(app_env.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut warnings = Vec::new();
    if let Err(e) = interpolate_values(&mut document, &vars, &mut warnings) {
        return Err(e);
    }
    let Value::Object(project) = document else {
        bail!("the top level must be a mapping");
    };
    split_project(project, warnings)
}

fn interpolate_values(
    value: &mut Value,
    vars: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    match value {
        Value::String(text) => {
            *text = match interpolate(text, vars, warnings) {
                Ok(expanded) => expanded,
                Err(e) => return Err(e),
            };
        }
        Value::Array(items) => {
            for item in items {
                if let Err(e) = interpolate_values(item, vars, warnings) {
                    return Err(e);
                }
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                if let Err(e) = interpolate_values(field, vars, warnings) {
                    return Err(e);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn split_project(mut project: Mapping, warnings: Vec<String>) -> Result<RawProject> {
    let name = match project.remove("name") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(name),
        Some(_) => bail!("top level 'name' must be a string"),
    };
    let services = match take_entries(&mut project, "services") {
        Ok(services) => services,
        Err(e) => return Err(e),
    };
    let volumes = match take_entries(&mut project, "volumes") {
        Ok(volumes) => volumes,
        Err(e) => return Err(e),
    };
    Ok(RawProject {
        services,
        volumes,
        name,
        warnings,
    })
}

fn take_entries(project: &mut Mapping, key: &str) -> Result<BTreeMap<String, Value>> {
    match project.remove(key) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Object(entries)) => Ok(entries.into_iter().collect()),
        Some(_) => bail!("top level '{key}' must be a mapping"),
    }
}

#[cfg(test)]
#[path = "raw_tests.rs"]
mod tests;

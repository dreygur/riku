//! The compose keys riku accepts.
//!
//! Anything outside the lists below is a deploy error naming the key, so a
//! compose feature riku does not implement (`deploy`, `privileged`, custom
//! networks) fails loudly instead of being ignored. The check runs on the
//! parsed document after merge keys are applied, so a key that arrives only
//! through `<<:` is refused too.

use super::{Mapping, Value};
use anyhow::{bail, Result};

/// Service level keys riku supports.
pub(crate) const SERVICE_KEYS: [&str; 17] = [
    "build",
    "command",
    "depends_on",
    "entrypoint",
    "env_file",
    "environment",
    "healthcheck",
    "image",
    "labels",
    "networks",
    "ports",
    "restart",
    "stop_grace_period",
    "stop_signal",
    "user",
    "volumes",
    "working_dir",
];

const TOP_LEVEL_KEYS: [&str; 5] = ["services", "version", "volumes", "networks", "name"];

/// Check the top level keys, the `networks` block and every service's keys.
pub(crate) fn check_keys(project: &Mapping) -> Result<()> {
    for key in project.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) && !key.starts_with("x-") {
            bail!("unsupported top level key '{key}'");
        }
    }
    if let Some(networks) = project.get("networks") {
        if let Err(e) = check_networks(networks) {
            return Err(e);
        }
    }
    match project.get("services") {
        None => Ok(()),
        Some(services) => check_services(services),
    }
}

fn check_networks(networks: &Value) -> Result<()> {
    let declared = match networks {
        Value::Null => return Ok(()),
        Value::Object(declared) => declared,
        _ => bail!("top level 'networks' must be a mapping"),
    };
    for (name, settings) in declared {
        if name != "default" {
            bail!(
                "network '{name}': riku gives each app one network; only 'default' may be declared"
            );
        }
        let empty = match settings {
            Value::Null => true,
            Value::Object(settings) => settings.is_empty(),
            _ => false,
        };
        if !empty {
            bail!("network 'default': riku does not support settings on the app network");
        }
    }
    Ok(())
}

fn check_services(services: &Value) -> Result<()> {
    let Value::Object(services) = services else {
        bail!("top level 'services' must be a mapping");
    };
    for (name, service) in services {
        let Value::Object(service) = service else {
            bail!("service '{name}': must be a mapping");
        };
        if let Some(key) = service
            .keys()
            .find(|key| !SERVICE_KEYS.contains(&key.as_str()))
        {
            bail!("service '{name}': unsupported key '{key}'");
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;

//! The top level `volumes` block.
//!
//! # Security model
//!
//! Engine volume names are shared by every app on the server, and a volume
//! name is only safe when riku builds it from the project name and the key.
//! `name:` and `external:` would let the compose file pick any name, so they
//! are refused, and `driver_opts` can turn a volume into a bind mount of a
//! host directory or device. Volume keys must be engine safe names.

use super::project::VolumeDecl;
use super::Value;
use crate::types::is_engine_safe;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// Check every declared volume and record it as [`VolumeDecl::Managed`].
pub(super) fn parse_volume_decls(
    declared: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, VolumeDecl>> {
    let mut volumes = BTreeMap::new();
    for (key, settings) in declared {
        if !is_engine_safe(key) {
            bail!("volume {key:?}: use letters, digits, '_', '.' or '-', starting with a letter or digit");
        }
        if let Err(e) = check_settings(key, settings) {
            return Err(e);
        }
        volumes.insert(key.clone(), VolumeDecl::Managed);
    }
    Ok(volumes)
}

fn check_settings(key: &str, settings: &Value) -> Result<()> {
    let fields = match settings {
        Value::Null => return Ok(()),
        Value::Object(fields) => fields,
        _ => bail!("volume '{key}' must be empty or a mapping"),
    };
    for (field, value) in fields {
        match (field.as_str(), value) {
            ("driver", Value::String(driver)) if driver == "local" => {}
            ("name" | "external" | "driver_opts", _) => bail!(
                "volume '{key}': '{field}' is not supported; riku names volumes from the project and the volume key"
            ),
            _ => bail!("volume '{key}': unsupported key or value for '{field}'"),
        }
    }
    Ok(())
}

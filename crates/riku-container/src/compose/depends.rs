//! The `depends_on` key.

use super::scalar::reject_unknown;
use super::{Dependency, DependsCondition, Value};
use crate::types::ServiceName;
use anyhow::{bail, Result};

/// Parse `depends_on` as a list of names or a map of name to `condition`.
/// Whether the named services exist is checked by the caller, which knows them
/// all.
pub(super) fn parse_depends_on(value: &Value) -> Result<Vec<Dependency>> {
    match value {
        Value::Array(names) => names
            .iter()
            .map(|name| match name {
                Value::String(name) => dependency(name, DependsCondition::Started),
                _ => bail!("depends_on entries must be service names"),
            })
            .collect(),
        Value::Object(entries) => entries
            .iter()
            .map(|(name, settings)| match parse_condition(name, settings) {
                Ok(condition) => dependency(name, condition),
                Err(e) => Err(e),
            })
            .collect(),
        _ => bail!("depends_on must be a list or a mapping"),
    }
}

fn dependency(name: &str, condition: DependsCondition) -> Result<Dependency> {
    match ServiceName::try_from(name) {
        Ok(service) => Ok(Dependency { service, condition }),
        Err(e) => Err(e),
    }
}

fn parse_condition(name: &str, settings: &Value) -> Result<DependsCondition> {
    let fields = match settings {
        Value::Null => return Ok(DependsCondition::Started),
        Value::Object(fields) => fields,
        _ => bail!("depends_on '{name}' must be empty or a mapping with a condition"),
    };
    if let Err(e) = reject_unknown(fields, &["condition"], &format!("depends_on '{name}'")) {
        return Err(e);
    }
    match fields.get("condition") {
        None => Ok(DependsCondition::Started),
        Some(Value::String(c)) if c == "service_started" => Ok(DependsCondition::Started),
        Some(Value::String(c)) if c == "service_healthy" => Ok(DependsCondition::Healthy),
        Some(other) => bail!(
            "depends_on '{name}' condition {other} is not supported; use service_started or service_healthy"
        ),
    }
}

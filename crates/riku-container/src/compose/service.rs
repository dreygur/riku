//! Parsing one compose service into a [`ServiceDef`].
//!
//! # Security model
//!
//! A service comes from a pushed compose file. [`parse_service`] checks every
//! field before it is stored: unknown keys are refused, ports bind loopback
//! unless the `riku.public` label says otherwise, paths stay inside the app
//! directory, and text that would reach the engine as an argument is checked
//! for option and separator injection. The per key checks live in
//! [`super::service_steps`].

use super::definition::{ServiceContext, ServiceDef};
use super::keys::SERVICE_KEYS;
use super::scalar::reject_unknown;
use super::service_steps::STEPS;
use super::{Mapping, Value};
use crate::types::{RestartPolicy, ServiceName, StopSignal, DEFAULT_STOP_GRACE};
use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;

/// Turn one raw service into a [`ServiceDef`]. Every error names the service.
pub fn parse_service(ctx: &ServiceContext, name: &str, value: &Value) -> Result<ServiceDef> {
    match build_service(ctx, name, value) {
        Ok(service) => Ok(service),
        Err(e) => Err(anyhow!("service '{name}': {e:#}")),
    }
}

fn build_service(ctx: &ServiceContext, name: &str, value: &Value) -> Result<ServiceDef> {
    let name = match ServiceName::try_from(name) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    let Value::Object(fields) = value else {
        bail!("must be a mapping");
    };
    // Compose extension keys (`x-...`) are accepted and ignored, as at the top
    // level.
    let known: Mapping = fields
        .iter()
        .filter(|(key, _)| !key.starts_with("x-"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if let Err(e) = reject_unknown(&known, &SERVICE_KEYS, "service") {
        return Err(e);
    }
    let mut service = ServiceDef {
        name,
        image: None,
        build: None,
        command: None,
        entrypoint: None,
        environment: BTreeMap::new(),
        ports: Vec::new(),
        mounts: Vec::new(),
        depends_on: Vec::new(),
        healthcheck: None,
        restart: RestartPolicy::No,
        stop_grace: DEFAULT_STOP_GRACE,
        stop_signal: StopSignal::default(),
        labels: BTreeMap::new(),
        working_dir: None,
        user: None,
    };
    for step in STEPS {
        if let Err(e) = step(ctx, fields, &mut service) {
            return Err(e);
        }
    }
    Ok(service)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

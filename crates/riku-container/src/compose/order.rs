//! Start order of a project's services.

use super::ServiceDef;
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

/// Order `services` so each comes after everything it depends on. Services
/// that are ready at the same time go by name, so the order is stable.
///
/// Refuses a dependency on a service the project does not define and a
/// dependency cycle, naming the services involved.
pub(super) fn start_order(services: Vec<ServiceDef>) -> Result<Vec<ServiceDef>> {
    let known: BTreeSet<String> = services.iter().map(|s| s.name.to_string()).collect();
    for service in &services {
        if let Some(missing) = service
            .depends_on
            .iter()
            .find(|dep| !known.contains(dep.service.as_str()))
        {
            bail!(
                "service '{}' depends on '{}', which the compose file does not define",
                service.name,
                missing.service
            );
        }
    }

    let mut waiting: BTreeMap<String, ServiceDef> = services
        .into_iter()
        .map(|service| (service.name.to_string(), service))
        .collect();
    let mut started = BTreeSet::new();
    let mut ordered = Vec::with_capacity(waiting.len());
    while !waiting.is_empty() {
        let ready = waiting
            .iter()
            .find(|(_, service)| {
                service
                    .depends_on
                    .iter()
                    .all(|dep| started.contains(dep.service.as_str()))
            })
            .map(|(name, _)| name.clone());
        let Some(name) = ready else {
            let stuck: Vec<&str> = waiting.keys().map(String::as_str).collect();
            bail!("dependency cycle among services: {}", stuck.join(", "));
        };
        if let Some(service) = waiting.remove(&name) {
            started.insert(name);
            ordered.push(service);
        }
    }
    Ok(ordered)
}

#[cfg(test)]
#[path = "order_tests.rs"]
mod tests;

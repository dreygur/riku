//! Reading `inspect` JSON into [`ContainerInfo`].
//!
//! Security model: the JSON is untrusted. Ids become [`ContainerId`], the app
//! label must pass [`check_app_name`] (the supervisor uses it as a path part),
//! service and copy labels become their validated types, and a missing or
//! malformed required field is an error, never a default. The one exception
//! is a listing, which leaves out a container whose app, service or copy label
//! does not parse (see [`BadLabels`]) so that one such container cannot break
//! every app.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use super::inspect::{ContainerInfo, ContainerState};
use super::ContainerId;
use crate::types::{check_app_name, labels, CopyIndex, ServiceName};

#[derive(Deserialize)]
struct RawContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Config")]
    config: RawConfig,
    #[serde(rename = "State")]
    state: RawState,
    #[serde(rename = "Image")]
    image_id: String,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct RawState {
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "ExitCode")]
    exit_code: Option<i64>,
    #[serde(rename = "Health")]
    health: Option<RawHealth>,
}

#[derive(Deserialize)]
struct RawHealth {
    #[serde(rename = "Status")]
    status: Option<String>,
}

fn container_state(state: &RawState) -> Result<ContainerState> {
    match state.status.as_str() {
        "running" => match state.health.as_ref().and_then(|h| h.status.as_deref()) {
            Some("unhealthy") => Ok(ContainerState::Unhealthy),
            Some("starting") => Ok(ContainerState::Starting),
            _ => Ok(ContainerState::Running),
        },
        "restarting" => Ok(ContainerState::Restarting),
        "created" => Ok(ContainerState::Created),
        "exited" => match state.exit_code.map(i32::try_from) {
            Some(Ok(code)) => Ok(ContainerState::Exited { code }),
            _ => Err(anyhow!("exited without a usable State.ExitCode")),
        },
        other => Ok(ContainerState::Other(other.to_string())),
    }
}

fn app_label(label_map: &HashMap<String, String>) -> Result<Option<String>> {
    match label_map.get(labels::APP) {
        None => Ok(None),
        Some(raw) => match check_app_name(raw) {
            Ok(()) => Ok(Some(raw.clone())),
            Err(e) => Err(anyhow!("label {}: {e}", labels::APP)),
        },
    }
}

fn service_label(label_map: &HashMap<String, String>) -> Result<Option<ServiceName>> {
    match label_map.get(labels::SERVICE) {
        None => Ok(None),
        Some(raw) => match ServiceName::try_from(raw.as_str()) {
            Ok(service) => Ok(Some(service)),
            Err(e) => Err(anyhow!("label {}: {e}", labels::SERVICE)),
        },
    }
}

fn copy_label(label_map: &HashMap<String, String>) -> Result<Option<CopyIndex>> {
    match label_map.get(labels::COPY) {
        None => Ok(None),
        Some(raw) => match raw.parse::<u32>().ok().and_then(CopyIndex::new) {
            Some(copy) => Ok(Some(copy)),
            None => Err(anyhow!(
                "label {} is {raw:?}, expected a number from 1",
                labels::COPY
            )),
        },
    }
}

fn port_label(label_map: &HashMap<String, String>, container: &str) -> Option<u16> {
    label_map
        .get(labels::PORT)
        .and_then(|raw| match raw.parse::<u16>() {
            Ok(port) if port != 0 => Some(port),
            _ => {
                tracing::warn!(
                    container,
                    label = labels::PORT,
                    "label is not a port number"
                );
                None
            }
        })
}

/// What to do with a container whose `riku.app`, `riku.service` or `riku.copy`
/// label does not parse.
#[derive(Debug, Clone, Copy)]
pub(super) enum BadLabels {
    /// Fail the whole read. For exact-name ownership checks, where an
    /// unreadable container must never count as free.
    Fail,
    /// Leave the container out and log a warning naming it. For listings.
    Skip,
}

/// Why a container could not be read.
enum Unreadable {
    Labels(anyhow::Error),
    Other(anyhow::Error),
}

struct RikuLabels {
    app: Option<String>,
    service: Option<ServiceName>,
    copy: Option<CopyIndex>,
}

#[allow(clippy::question_mark)]
fn riku_labels(label_map: &HashMap<String, String>) -> Result<RikuLabels> {
    let app = match app_label(label_map) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let service = match service_label(label_map) {
        Ok(service) => service,
        Err(e) => return Err(e),
    };
    match copy_label(label_map) {
        Ok(copy) => Ok(RikuLabels { app, service, copy }),
        Err(e) => Err(e),
    }
}

fn into_info(raw: RawContainer) -> std::result::Result<ContainerInfo, Unreadable> {
    let name = raw.name.trim_start_matches('/').to_string();
    let id = match ContainerId::try_from(raw.id.as_str()) {
        Ok(id) => id,
        Err(e) => return Err(Unreadable::Other(anyhow!("container {name}: {e}"))),
    };
    let state = match container_state(&raw.state) {
        Ok(state) => state,
        Err(e) => return Err(Unreadable::Other(anyhow!("container {name}: {e}"))),
    };
    let label_map = raw.config.labels.unwrap_or_default();
    let riku = match riku_labels(&label_map) {
        Ok(parsed) => parsed,
        Err(e) => return Err(Unreadable::Labels(anyhow!("container {name}: {e}"))),
    };
    let host_port = port_label(&label_map, &name);
    Ok(ContainerInfo {
        id,
        app: riku.app,
        restart: label_map.get(labels::RESTART).cloned(),
        host_port,
        name,
        service: riku.service,
        copy: riku.copy,
        image: raw.config.image,
        image_id: raw.image_id,
        state,
    })
}

pub(super) fn parse_inspect(json: &str, bad_labels: BadLabels) -> Result<Vec<ContainerInfo>> {
    let raws: Vec<RawContainer> = match serde_json::from_str(json) {
        Ok(raws) => raws,
        Err(e) => {
            return Err(anyhow!(
                "engine inspect output is not the expected JSON: {e}"
            ))
        }
    };
    let mut infos = Vec::with_capacity(raws.len());
    for raw in raws {
        match (into_info(raw), bad_labels) {
            (Ok(info), _) => infos.push(info),
            (Err(Unreadable::Labels(e)), BadLabels::Skip) => {
                tracing::warn!("skipping a container whose riku labels do not parse: {e:#}");
            }
            (Err(Unreadable::Labels(e) | Unreadable::Other(e)), _) => return Err(e),
        }
    }
    Ok(infos)
}

#[cfg(test)]
#[path = "inspect_parse_tests.rs"]
mod tests;

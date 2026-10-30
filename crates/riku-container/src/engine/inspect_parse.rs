//! Reading `inspect` JSON into [`ContainerInfo`].
//!
//! Security model: the JSON is untrusted. Ids become [`ContainerId`], service
//! and copy labels their validated types, and a missing or malformed required
//! field is an error, never a default.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use super::inspect::{ContainerInfo, ContainerState};
use super::ContainerId;
use crate::types::{labels, CopyIndex, ServiceName};

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
        "running" => {
            let health = state.health.as_ref().and_then(|h| h.status.as_deref());
            if health == Some("unhealthy") {
                Ok(ContainerState::Unhealthy)
            } else {
                Ok(ContainerState::Running)
            }
        }
        "restarting" => Ok(ContainerState::Restarting),
        "created" => Ok(ContainerState::Created),
        "exited" => match state.exit_code.map(i32::try_from) {
            Some(Ok(code)) => Ok(ContainerState::Exited { code }),
            _ => Err(anyhow!("exited without a usable State.ExitCode")),
        },
        other => Ok(ContainerState::Other(other.to_string())),
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

fn into_info(raw: RawContainer) -> Result<ContainerInfo> {
    let name = raw.name.trim_start_matches('/').to_string();
    let id = match ContainerId::try_from(raw.id.as_str()) {
        Ok(id) => id,
        Err(e) => return Err(anyhow!("container {name}: {e}")),
    };
    let state = match container_state(&raw.state) {
        Ok(state) => state,
        Err(e) => return Err(anyhow!("container {name}: {e}")),
    };
    let label_map = raw.config.labels.unwrap_or_default();
    let service = match service_label(&label_map) {
        Ok(service) => service,
        Err(e) => return Err(anyhow!("container {name}: {e}")),
    };
    let copy = match copy_label(&label_map) {
        Ok(copy) => copy,
        Err(e) => return Err(anyhow!("container {name}: {e}")),
    };
    Ok(ContainerInfo {
        id,
        name,
        service,
        copy,
        image: raw.config.image,
        state,
    })
}

pub(super) fn parse_inspect(json: &str) -> Result<Vec<ContainerInfo>> {
    let raws: Vec<RawContainer> = match serde_json::from_str(json) {
        Ok(raws) => raws,
        Err(e) => {
            return Err(anyhow!(
                "engine inspect output is not the expected JSON: {e}"
            ))
        }
    };
    raws.into_iter().map(into_info).collect()
}

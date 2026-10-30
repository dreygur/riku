//! Container state as the engine reports it.
//!
//! Security model: engine output is untrusted. Ids are parsed into
//! [`ContainerId`], service and copy labels into their validated types, and a
//! missing or malformed required field is an error, never a default. The app
//! name that selects containers must pass the same check as a container name
//! before it reaches the `--filter` argument.

use std::collections::HashMap;
use std::ffi::OsString;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use super::{ContainerId, Engine};
use crate::types::{check_app_name, labels, CopyIndex, ServiceName};

/// Lifecycle state of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    /// Running, and either has no health check or it has not failed.
    Running,
    /// Running, but the engine's health check reports `unhealthy`.
    Unhealthy,
    /// The engine is in the middle of restarting it.
    Restarting,
    /// Stopped, with the exit code of its main process.
    Exited { code: i32 },
    /// Created and never started.
    Created,
    /// Any other status the engine reports, for example `paused`.
    Other(String),
}

/// What riku needs to know about one container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    /// Full container id.
    pub id: ContainerId,
    /// Container name without the leading slash docker adds.
    pub name: String,
    /// From the `riku.service` label; `None` when the container has none.
    pub service: Option<ServiceName>,
    /// From the `riku.copy` label; `None` when the container has none.
    pub copy: Option<CopyIndex>,
    /// Image reference the container was created from, as written at creation.
    pub image: String,
    pub state: ContainerState,
}

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

pub(crate) fn parse_inspect(json: &str) -> Result<Vec<ContainerInfo>> {
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

fn parse_ids(output: &str) -> Result<Vec<ContainerId>> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ContainerId::try_from)
        .collect()
}

impl Engine {
    /// Full ids of every container of `app`, running or not.
    ///
    /// Selection is by the `riku.app` label, so containers riku did not create
    /// are never returned.
    pub fn list(&self, app: &str) -> Result<Vec<ContainerId>> {
        match check_app_name(app) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        let filter = format!("label={}={app}", labels::APP);
        let args: Vec<OsString> = ["ps", "-a", "-q", "--no-trunc", "--filter", &filter]
            .into_iter()
            .map(OsString::from)
            .collect();
        let stdout = match self.output(&args) {
            Ok(stdout) => stdout,
            Err(e) => return Err(anyhow!("cannot list containers of {app}: {e}")),
        };
        parse_ids(&stdout)
    }

    /// State of each container in `ids`. Returns an empty list without running
    /// the engine when `ids` is empty.
    pub fn inspect(&self, ids: &[ContainerId]) -> Result<Vec<ContainerInfo>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<OsString> = ["inspect", "--type", "container"]
            .into_iter()
            .map(OsString::from)
            .collect();
        args.extend(ids.iter().map(|id| OsString::from(id.as_str())));
        let stdout = match self.output(&args) {
            Ok(stdout) => stdout,
            Err(e) => return Err(anyhow!("cannot inspect containers: {e}")),
        };
        parse_inspect(&stdout)
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;

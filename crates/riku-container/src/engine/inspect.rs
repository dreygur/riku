//! Container state as the engine reports it.
//!
//! Security model: the app name that selects containers must pass the same
//! check as a container name before it reaches the `--filter` argument, and
//! engine output is parsed by [`super::inspect_parse`], which treats it as
//! untrusted.

use std::ffi::OsString;

use anyhow::{anyhow, Result};

use super::inspect_parse::parse_inspect;
use super::snapshot::fetch_present;
use super::{ContainerId, Engine};
use crate::types::{check_app_name, labels, CopyIndex, ServiceName};

/// Lifecycle state of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    /// Running, and either has no health check or it has not failed.
    Running,
    /// Running, but the engine's health check has not passed yet (`starting`).
    Starting,
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
    /// From the `riku.app` label; `None` when the container has none.
    pub app: Option<String>,
    /// From the `riku.service` label; `None` when the container has none.
    pub service: Option<ServiceName>,
    /// From the `riku.copy` label; `None` when the container has none.
    pub copy: Option<CopyIndex>,
    /// Image reference the container was created from, as written at creation.
    pub image: String,
    /// Engine id of the image the container runs, as `inspect` reports it
    /// (docker prefixes it with `sha256:`).
    pub image_id: String,
    /// Host port from the `riku.port` label; `None` when the label is absent
    /// or is not a port number.
    pub host_port: Option<u16>,
    /// The `riku.restart` label as written; `None` when absent.
    pub restart: Option<String>,
    pub state: ContainerState,
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
    pub(super) fn list_with_filter(&self, filter: Option<&str>) -> Result<Vec<ContainerId>> {
        let mut args: Vec<OsString> = ["ps", "-a", "-q", "--no-trunc"]
            .into_iter()
            .map(OsString::from)
            .collect();
        if let Some(filter) = filter {
            args.extend(["--filter", filter].map(OsString::from));
        }
        match self.output(&args) {
            Ok(stdout) => parse_ids(&stdout),
            Err(e) => Err(anyhow!("cannot list containers: {e}")),
        }
    }

    /// Full ids of every container the engine knows, riku's or not.
    pub(super) fn list_all_containers(&self) -> Result<Vec<ContainerId>> {
        self.list_with_filter(None)
    }

    /// Full ids of every container of `app`, running or not.
    ///
    /// Selection is by the `riku.app` label, so containers riku did not create
    /// are never returned.
    pub fn list(&self, app: &str) -> Result<Vec<ContainerId>> {
        match check_app_name(app) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        self.list_with_filter(Some(&format!("label={}={app}", labels::APP)))
    }

    /// State of each container in `ids`.
    ///
    /// Returns an empty list without running the engine when `ids` is empty.
    /// A container removed after it was listed is left out of the result; any
    /// other failure is an error. See [`super::snapshot`] for how the two are
    /// told apart.
    pub fn inspect(&self, ids: &[ContainerId]) -> Result<Vec<ContainerInfo>> {
        let run = |ids: &[ContainerId]| {
            let mut args: Vec<OsString> = ["inspect", "--type", "container"]
                .into_iter()
                .map(OsString::from)
                .collect();
            args.extend(ids.iter().map(|id| OsString::from(id.as_str())));
            self.output(&args)
        };
        match fetch_present(self, ids, run, parse_inspect) {
            Ok(infos) => Ok(infos),
            Err(e) => Err(anyhow!("cannot inspect containers: {e}")),
        }
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;

//! [`CopyRuntime`] for the containers of one compose service.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::compose::{ComposeProject, ServiceDef};
use riku_container::engine::{ContainerId, ContainerInfo, ContainerState, Engine};
use riku_container::types::CopyIndex;
use riku_util::backend::Backend;

use super::{copy_spec, pick_free_port, start_copy, AppName};
use crate::rolling::health::{health_target, wait_healthy};
use crate::rolling::{CopyHandle, CopyRuntime, LiveCopy};

/// Starts, probes and stops the copies of `service`. A routed service's
/// copies each get their own loopback host port, which is their backend.
pub(crate) struct ContainerRuntime<'a> {
    pub engine: &'a Engine,
    pub app: &'a AppName,
    pub project: &'a ComposeProject,
    pub service: &'a ServiceDef,
    pub app_path: &'a Path,
    pub scratch: &'a Path,
    pub env: &'a BTreeMap<String, String>,
    pub routed: bool,
}

fn container_of(copy: &LiveCopy) -> Result<&ContainerId> {
    match &copy.handle {
        CopyHandle::Container(id) => Ok(id),
        CopyHandle::Process(name) => Err(anyhow!("{name} is a process, not a container")),
    }
}

fn host_port(copy: &LiveCopy) -> Option<u16> {
    match copy.backend {
        Some(Backend::Tcp { port, .. }) => Some(port),
        _ => None,
    }
}

impl ContainerRuntime<'_> {
    fn live_copy(&self, info: ContainerInfo) -> Option<LiveCopy> {
        let copy = match info.copy {
            Some(copy) => copy,
            None => return None,
        };
        let backend = match (self.routed, &info.state, info.host_port) {
            (true, ContainerState::Running, Some(port)) => Some(Backend::loopback(port)),
            _ => None,
        };
        Some(LiveCopy {
            copy,
            backend,
            handle: CopyHandle::Container(info.id),
        })
    }

    fn check_not_exited(&self, id: &ContainerId) -> Result<()> {
        let infos = match self.engine.inspect(std::slice::from_ref(id)) {
            Ok(infos) => infos,
            Err(e) => return Err(e),
        };
        match infos.first().map(|info| &info.state) {
            Some(ContainerState::Exited { code }) => Err(anyhow!("exited with code {code}")),
            Some(_) => Ok(()),
            None => Err(anyhow!("container {} is gone", id.as_str())),
        }
    }

    fn stop_and_remove(&self, id: &ContainerId) -> Result<()> {
        if let Err(e) = self.engine.stop(id, self.service.stop_grace) {
            return Err(e);
        }
        self.engine.remove(id)
    }
}

impl CopyRuntime for ContainerRuntime<'_> {
    /// Every container of the service, running or not, so a dead copy's slot
    /// is replaced too. Only running copies of a routed service have a
    /// backend.
    fn live(&self) -> Result<Vec<LiveCopy>> {
        let ids = match self.engine.list(self.app.as_str()) {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };
        let infos = match self.engine.inspect(&ids) {
            Ok(infos) => infos,
            Err(e) => return Err(e),
        };
        Ok(infos
            .into_iter()
            .filter(|info| info.service.as_ref() == Some(&self.service.name))
            .filter_map(|info| self.live_copy(info))
            .collect())
    }

    fn start(&mut self, copy: CopyIndex) -> Result<LiveCopy> {
        let port = if self.routed {
            match pick_free_port() {
                Ok(port) => Some(port),
                Err(e) => return Err(e),
            }
        } else {
            None
        };
        let spec = match copy_spec(self.app, self.project, self.service, copy, port, self.env) {
            Ok(spec) => spec,
            Err(e) => return Err(e),
        };
        let id = match start_copy(self.engine, &spec, self.app_path, self.scratch) {
            Ok(id) => id,
            Err(e) => return Err(e),
        };
        Ok(LiveCopy {
            copy,
            backend: port.map(Backend::loopback),
            handle: CopyHandle::Container(id),
        })
    }

    /// A service with no health check and no routed port only has to still
    /// be running once.
    fn wait_healthy(&self, copy: &LiveCopy) -> Result<()> {
        let id = match container_of(copy) {
            Ok(id) => id,
            Err(e) => return Err(e),
        };
        match health_target(self.service, id, host_port(copy)) {
            Some(target) => wait_healthy(&target, Some(self.engine)),
            None => self.check_not_exited(id),
        }
    }

    fn stop(&mut self, copy: &LiveCopy) -> Result<()> {
        match container_of(copy) {
            Ok(id) => self.stop_and_remove(id),
            Err(e) => Err(e),
        }
    }

    fn discard(&mut self, copy: &LiveCopy) {
        let outcome = match container_of(copy) {
            Ok(id) => self.stop_and_remove(id),
            Err(e) => Err(e),
        };
        if let Err(e) = outcome {
            tracing::warn!(
                service = %self.service.name,
                copy = %copy.copy,
                "could not remove an unhealthy copy: {e}"
            );
        }
    }
}

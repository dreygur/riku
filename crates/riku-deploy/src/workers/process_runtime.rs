//! [`CopyRuntime`] for the supervised copies of one web facing Procfile kind.
//!
//! A copy is an enabled `<app>@<kind>.<n>.toml` worker config. Starting one
//! writes the config into a free slot and the supervisor spawns it; stopping
//! one removes the enabled symlink and waits for the supervisor to report the
//! process gone. An existing config is never rewritten, so the supervisor's
//! in place canary path is not involved.
//!
//! Security: worker configs are riku's own files. Ports and sockets are read
//! back through typed parsing, and nothing from the app env reaches a shell
//! here.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use riku_container::types::CopyIndex;
use riku_supervisor::config::WorkerConfig;
use riku_util::backend::Backend;
use riku_util::file_names::worker::WorkerName;

use super::files::write_worker_config;
use super::ports::enabled_ports;
use super::stop_wait::wait_stopped;
use super::web_env::build_worker_env;
use crate::config::RikuPaths;
use crate::rolling::health::{procfile_target, wait_healthy, HealthTarget};
use crate::rolling::{CopyHandle, CopyRuntime, LiveCopy};

/// How long past a copy's grace period a stop waits for the supervisor.
const STOP_MARGIN: Duration = Duration::from_secs(5);

/// Starts, probes and stops the copies of `kind`, a web facing Procfile kind
/// (`web` or a wsgi variant).
pub(crate) struct ProcessRuntime<'a> {
    pub app: &'a str,
    pub app_path: &'a Path,
    pub kind: &'a str,
    pub command: &'a str,
    pub env: &'a HashMap<String, String>,
    pub paths: &'a RikuPaths,
}

impl ProcessRuntime<'_> {
    fn worker_name(&self, copy: CopyIndex) -> WorkerName {
        WorkerName::new(self.app, self.kind, copy.get())
    }

    fn live_copy(&self, config: &WorkerConfig) -> Result<Option<LiveCopy>> {
        let copy = match CopyIndex::new(config.worker.ordinal) {
            Some(copy) => copy,
            None => {
                tracing::warn!(
                    app = self.app,
                    kind = self.kind,
                    "skipping a copy numbered 0"
                );
                return Ok(None);
            }
        };
        match crate::router::worker_backend(config) {
            Ok(backend) => Ok(Some(LiveCopy {
                copy,
                backend: Some(backend),
                handle: CopyHandle::Process(format!("{}-{copy}", self.kind)),
            })),
            Err(e) => Err(e.context(format!("{} copy {copy} has no usable address", self.kind))),
        }
    }

    fn grace_period(&self, enabled: &Path) -> Duration {
        let configured = fs::read_to_string(enabled)
            .ok()
            .and_then(|text| toml::from_str::<WorkerConfig>(&text).ok())
            .map(|config| config.options.grace_period)
            .unwrap_or(riku_config::RIKU_WORKER_GRACE_PERIOD);
        Duration::from_secs(configured)
    }
}

impl CopyRuntime for ProcessRuntime<'_> {
    fn live(&self) -> Result<Vec<LiveCopy>> {
        let configs = match crate::router::web_copies(self.app, self.paths) {
            Ok(configs) => configs,
            Err(e) => return Err(e),
        };
        let mut live = Vec::new();
        for config in configs.iter().filter(|c| c.worker.kind == self.kind) {
            match self.live_copy(config) {
                Ok(Some(copy)) => live.push(copy),
                Ok(None) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(live)
    }

    fn start(&mut self, copy: CopyIndex) -> Result<LiveCopy> {
        let taken = match enabled_ports(self.app, self.paths) {
            Ok(taken) => taken,
            Err(e) => return Err(e),
        };
        let env = match build_worker_env(self.app, self.kind, copy, self.env, &taken, self.paths) {
            Ok(env) => env,
            Err(e) => return Err(e),
        };
        let written = match write_worker_config(
            self.app,
            self.app_path,
            self.kind,
            self.command,
            copy.get(),
            env,
            self.paths,
        ) {
            Ok(written) => written,
            Err(e) => return Err(e),
        };
        crate::supervisor_ctl::notify_supervisor_reload(self.paths);
        match self.live_copy(&written) {
            Ok(Some(live)) => Ok(live),
            Ok(None) => Err(anyhow!("{} copy {copy} has no copy number", self.kind)),
            Err(e) => Err(e),
        }
    }

    fn wait_healthy(&self, copy: &LiveCopy) -> Result<()> {
        let target = match &copy.backend {
            Some(Backend::Tcp { port, .. }) => procfile_target(self.env, *port),
            Some(Backend::Unix(path)) => HealthTarget::Socket(path.clone()),
            None => return Err(anyhow!("{} copy {} has no address", self.kind, copy.copy)),
        };
        wait_healthy(&target, None)
    }

    fn stop(&mut self, copy: &LiveCopy) -> Result<()> {
        let name = self.worker_name(copy.copy);
        let process_id = name.to_string();
        let enabled = self.paths.workers_enabled.join(name.file_name());
        let grace = self.grace_period(&enabled);
        match fs::remove_file(&enabled) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(anyhow!("cannot disable {}: {e}", enabled.display())),
        }
        crate::supervisor_ctl::notify_supervisor_reload(self.paths);
        wait_stopped(self.paths, &process_id, grace + STOP_MARGIN);
        Ok(())
    }

    fn discard(&mut self, copy: &LiveCopy) {
        if let Err(e) = self.stop(copy) {
            tracing::warn!(
                kind = self.kind,
                copy = %copy.copy,
                "could not stop an unhealthy copy: {e}"
            );
        }
    }
}

#[cfg(test)]
#[path = "process_runtime_tests.rs"]
mod tests;

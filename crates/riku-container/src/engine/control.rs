//! Starting, stopping, building and following containers.
//!
//! Security model: every value is one argv element. Container ids are
//! [`ContainerId`]s (hex only). Images and build contexts, which are
//! positional, are refused when they could be read as a flag. Environment
//! values reach the engine through an env file in the caller's scratch
//! directory (see [`super::env_file`]) that is deleted before `run` returns.
//! Removal never passes a volume flag.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Result};
use riku_config::settings::EngineSettings;

use super::env_file::write_env_file;
use super::run_args::{run_args, whole_seconds_up};
use super::run_spec::{PullPolicy, RunSpec};
use super::spec_check::{check_positional, check_spec};
use super::{ContainerId, Engine};
use crate::types::StopSignal;

fn args_of<const N: usize>(parts: [&str; N]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
}

fn run_limit(settings: &EngineSettings, pull: PullPolicy) -> Duration {
    match pull {
        PullPolicy::Missing => settings.pull_timeout,
        PullPolicy::Never => settings.call_timeout,
    }
}

impl Engine {
    /// Start `spec` detached and return its id.
    ///
    /// The env file is written under `scratch` and removed when this returns.
    /// A spec that may pull a missing image gets the pull limit, since the
    /// pull happens inside this call; one that never pulls gets the quick
    /// call limit.
    pub fn run(&self, spec: &RunSpec, scratch: &Path) -> Result<ContainerId> {
        if let Err(e) = check_spec(spec) {
            return Err(anyhow!("cannot run {}: {e}", spec.name.as_str()));
        }
        let env_file = match write_env_file(scratch, &spec.env) {
            Ok(file) => file,
            Err(e) => return Err(anyhow!("cannot run {}: {e}", spec.name.as_str())),
        };
        let args = match run_args(spec, &env_file) {
            Ok(args) => args,
            Err(e) => return Err(anyhow!("cannot run {}: {e}", spec.name.as_str())),
        };
        let limit = run_limit(self.settings(), spec.pull);
        let stdout = match self.output_within(&args, limit) {
            Ok(stdout) => stdout,
            Err(e) => return Err(anyhow!("cannot start {}: {e}", spec.name.as_str())),
        };
        ContainerId::try_from(stdout.trim())
    }

    /// Send the stop signal, then kill after `grace`, rounded up to a whole
    /// second. The engine call itself is given `stop_extra` more than the
    /// grace before riku gives up on it.
    pub fn stop(&self, id: &ContainerId, grace: Duration) -> Result<()> {
        let secs = whole_seconds_up(grace);
        let limit = Duration::from_secs(secs) + self.settings().stop_extra;
        self.output_within(
            &args_of(["stop", "-t", &secs.to_string(), id.as_str()]),
            limit,
        )
        .map(|_| ())
    }

    /// Send `signal` to the container's main process.
    pub fn kill(&self, id: &ContainerId, signal: &StopSignal) -> Result<()> {
        self.output(&args_of(["kill", "--signal", signal.as_str(), id.as_str()]))
            .map(|_| ())
    }

    /// Remove the container. Volumes are left alone.
    pub fn remove(&self, id: &ContainerId) -> Result<()> {
        self.output(&args_of(["rm", id.as_str()])).map(|_| ())
    }

    /// Pull `image` from its registry, streaming the engine's output and
    /// stopping it at the pull limit.
    pub fn pull(&self, image: &str) -> Result<()> {
        match check_positional("image", image) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        self.run_streamed(
            &args_of(["pull", image]),
            image,
            self.settings().pull_timeout,
        )
    }

    /// A `logs` command for the caller to spawn and stream. `tail` limits the
    /// history to that many lines.
    pub fn logs_command(&self, id: &ContainerId, follow: bool, tail: Option<u32>) -> Command {
        let mut command = Command::new(self.program());
        command.arg("logs");
        if follow {
            command.arg("-f");
        }
        if let Some(lines) = tail {
            command.args(["--tail", &lines.to_string()]);
        }
        command.arg(id.as_str());
        command
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;

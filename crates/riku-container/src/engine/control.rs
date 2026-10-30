//! Starting, stopping, building and following containers.
//!
//! Security model: every value is one argv element. Container ids are
//! [`ContainerId`]s (hex only). Images and build contexts, which are
//! positional, are refused when they could be read as a flag. Environment
//! values reach the engine through an env file in the caller's scratch
//! directory (see [`super::env_file`]) that is deleted before `run` returns.
//! Removal never passes a volume flag.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Result};

use super::env_file::write_env_file;
use super::run_args::{run_args, RunSpec};
use super::spec_check::{check_positional, check_spec};
use super::{ContainerId, Engine};
use crate::types::StopSignal;

/// An image to build from a directory.
///
/// Build arguments are passed on the command line, so they are visible in the
/// process list while the build runs. Do not put secrets in them.
#[derive(Debug, Clone)]
pub struct BuildSpec {
    pub context: PathBuf,
    /// Dockerfile path; `None` uses the engine's default in the context.
    pub dockerfile: Option<PathBuf>,
    pub tag: String,
    pub args: BTreeMap<String, String>,
    pub platform: Option<String>,
}

fn args_of<const N: usize>(parts: [&str; N]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
}

fn build_args(spec: &BuildSpec) -> Vec<OsString> {
    let mut args = args_of(["build", "-t", &spec.tag]);
    if let Some(dockerfile) = &spec.dockerfile {
        args.push("-f".into());
        args.push(dockerfile.into());
    }
    for (key, value) in &spec.args {
        args.push("--build-arg".into());
        args.push(format!("{key}={value}").into());
    }
    if let Some(platform) = &spec.platform {
        args.push("--platform".into());
        args.push(platform.into());
    }
    args.push((&spec.context).into());
    args
}

impl Engine {
    /// Start `spec` detached and return its id.
    ///
    /// The env file is written under `scratch` and removed when this returns.
    pub fn run(&self, spec: &RunSpec, scratch: &Path) -> Result<ContainerId> {
        if let Err(e) = check_spec(spec) {
            return Err(anyhow!("cannot run {}: {e}", spec.name.as_str()));
        }
        let env_file = match write_env_file(scratch, &spec.env) {
            Ok(file) => file,
            Err(e) => return Err(anyhow!("cannot run {}: {e}", spec.name.as_str())),
        };
        let stdout = match self.output(&run_args(spec, &env_file)) {
            Ok(stdout) => stdout,
            Err(e) => return Err(anyhow!("cannot start {}: {e}", spec.name.as_str())),
        };
        ContainerId::try_from(stdout.trim())
    }

    /// Send the stop signal, then kill after `grace`.
    pub fn stop(&self, id: &ContainerId, grace: Duration) -> Result<()> {
        let secs = grace.as_secs().to_string();
        self.output(&args_of(["stop", "-t", &secs, id.as_str()]))
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

    /// Pull `image` from its registry.
    pub fn pull(&self, image: &str) -> Result<()> {
        match check_positional("image", image) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        self.output(&args_of(["pull", image])).map(|_| ())
    }

    /// Build an image from `spec.context`.
    pub fn build(&self, spec: &BuildSpec) -> Result<()> {
        match check_positional("build context", &spec.context.to_string_lossy()) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        self.output(&build_args(spec)).map(|_| ())
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

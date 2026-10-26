//! Invoking compose through whichever CLI the host has.

use anyhow::{anyhow, bail, Result};
use std::path::Path;
use std::process::Command;

use crate::compose_file::ComposeFile;
use crate::{runtime, which};

/// Program and leading args for invoking compose with the active container runtime.
pub(crate) fn compose_cmd() -> (&'static str, Vec<&'static str>) {
    if runtime() == "podman" {
        if which("podman-compose") {
            return ("podman-compose", vec![]);
        }
        return ("podman", vec!["compose"]);
    }
    ("docker", vec!["compose"])
}

pub(crate) fn compose_cmd_display(
    (program, base_args): &(&'static str, Vec<&'static str>),
) -> String {
    if base_args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, base_args.join(" "))
    }
}

/// `compose pull` arguments that leave services with a `build:` section to
/// `compose build`.
///
/// docker compose fails the whole pull when a buildable service's image is in
/// no registry, so it needs `--ignore-buildable`. podman-compose already skips
/// those services and has no such flag. `podman compose` is only picked when
/// podman-compose is not on PATH, so it delegates to docker compose and takes
/// the flag.
fn pull_args(program: &str) -> &'static [&'static str] {
    if program == "podman-compose" {
        &["pull"]
    } else {
        &["pull", "--ignore-buildable"]
    }
}

/// Run one compose subcommand against the app's compose file, failing with
/// `step` in the message if it can't start or exits non-zero.
pub(crate) fn run_compose(
    app_path: &Path,
    compose: &ComposeFile,
    args: &[&str],
    step: &str,
) -> Result<()> {
    let (program, base_args) = compose_cmd();
    let status = match Command::new(program)
        .args(&base_args)
        .args(["-f", &compose.arg()])
        .args(args)
        .current_dir(app_path)
        .status()
    {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("failed to run {step}: {e}")),
    };
    if !status.success() {
        bail!("{step} failed");
    }
    Ok(())
}

/// Pull the registry images, then build every service that has a `build:`
/// section. `compose build` is a no-op for a stack of pre-built images.
#[allow(clippy::question_mark)]
pub(crate) fn pull_and_build(app_path: &Path, compose: &ComposeFile) -> Result<()> {
    let (program, _) = compose_cmd();
    if let Err(e) = run_compose(app_path, compose, pull_args(program), "compose pull") {
        return Err(e);
    }
    run_compose(app_path, compose, &["build"], "compose build")
}

#[cfg(test)]
#[path = "compose_cli_tests.rs"]
mod tests;

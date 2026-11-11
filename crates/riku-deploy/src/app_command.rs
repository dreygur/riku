//! The Procfile's one shot commands, `preflight` and `release`, run in the
//! app's checkout with the worker resource limits. Their output streams to
//! the pusher under the output rail.
//!
//! Each command runs in its own process group, killed once the command
//! exits, so a background child cannot keep the deploy (and its lock) open.
//! There is no time limit: a long migration is a legitimate release.
//!
//! A failure is returned, never an exit of the process: the hook must live
//! long enough to report it to `git push`, and the dashboard runs deploys
//! inside its own server process.

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;

use crate::error::DeployError;
use crate::plugins::executor::{classify_resource_exit, exit_code_for, tee_output};
use crate::supervisor::resource_limits::ResourceLimits;
use crate::util::display::OutputTail;

/// Which one shot command runs, as named in errors.
#[derive(Clone, Copy, Debug)]
pub(crate) enum OneShot {
    Preflight,
    Release,
}

impl OneShot {
    fn name(self) -> &'static str {
        match self {
            OneShot::Preflight => "preflight",
            OneShot::Release => "release",
        }
    }
}

/// Run `command` with `sh -c` in `app_path`. A command stopped by a resource
/// limit gets the structured diagnostic instead of a bare exit code.
pub(crate) fn run_one_shot(
    kind: OneShot,
    command: &str,
    app_path: &Path,
    recent: &OutputTail,
) -> Result<()> {
    let limits = ResourceLimits::from_env();
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(app_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    unsafe {
        cmd.pre_exec(move || limits.apply());
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return Err(anyhow!("cannot run the {} command: {e}", kind.name())),
    };
    let (tee_handles, stderr_tail) = tee_output(&mut child, recent);
    let waited = child.wait();
    // Background children the command left behind would hold the output
    // pipes open, so the readers below would never see end of file.
    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
    let status = match waited {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("cannot wait for the {} command: {e}", kind.name())),
    };
    for handle in tee_handles {
        let _ = handle.join();
    }
    if status.success() {
        return Ok(());
    }

    let tail = match stderr_tail.lock() {
        Ok(tail) => tail.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    if let Some(cause) = classify_resource_exit(&status, &tail) {
        return Err(DeployError::resource_exhausted(kind.name(), command, &cause).into());
    }
    Err(anyhow!(
        "the {} command exited with code {}",
        kind.name(),
        exit_code_for(&status)
    ))
}

#[cfg(test)]
#[path = "app_command_tests.rs"]
mod tests;

//! The two commands the helper runs against the real nginx: the syntax check
//! and the reload. Programs and arguments are fixed, nothing from a request
//! reaches a command line.
//!
//! The check is the plain `nginx -t`, so it reads the host's own nginx.conf
//! and the site files in place, exactly what a reload would load. Only the
//! exit status counts. The deploy side's wrapper accepted any output that
//! said "syntax is ok"; as root there is no permission error to excuse.

use std::process::{Command, Output};

use anyhow::{anyhow, Result};
use riku_util::spawn::output_retrying_etxtbsy;

use crate::nginx_roots::{NginxRoots, ReloadMethod};

fn run(command: &mut Command, what: &str) -> Result<()> {
    let output: Output = match output_retrying_etxtbsy(command) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("could not run {what}: {e}")),
    };
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "{what} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Have nginx check the config as it now stands on disk.
pub(crate) fn check_config(roots: &NginxRoots) -> Result<()> {
    run(Command::new(&roots.nginx).arg("-t"), "nginx -t")
}

/// Have nginx load the config on disk without dropping connections.
pub(crate) fn reload(roots: &NginxRoots) -> Result<()> {
    match &roots.reload {
        ReloadMethod::Systemctl(systemctl) => run(
            Command::new(systemctl).args(["reload", "nginx"]),
            "systemctl reload nginx",
        ),
        ReloadMethod::Signal => run(
            Command::new(&roots.nginx).args(["-s", "reload"]),
            "nginx -s reload",
        ),
    }
}

//! Registry login.
//!
//! Security model: the token is written to the engine's stdin and never
//! appears in argv, where any local user could read it from the process list.
//! Registry and user are refused when they could be read as a flag. Engine
//! stderr is returned in the error, and engines do not echo the token.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use riku_util::spawn::spawn_retrying_etxtbsy;

use super::spec_check::check_positional;
use super::Engine;

impl Engine {
    /// Log the engine in to `registry` with `user` and `token`.
    pub fn login(&self, registry: &str, user: &str, token: &str) -> Result<()> {
        for (what, value) in [("registry", registry), ("registry user", user)] {
            match check_positional(what, value) {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        if token.is_empty() {
            return Err(anyhow!("registry token is empty"));
        }
        let mut command = Command::new(self.program());
        command
            .args(["login", "--username", user, "--password-stdin", registry])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = match spawn_retrying_etxtbsy(&mut command) {
            Ok(child) => child,
            Err(e) => return Err(anyhow!("cannot run 'login' on the container engine: {e}")),
        };
        let written = match child.stdin.take() {
            Some(mut stdin) => stdin.write_all(token.as_bytes()),
            None => Ok(()),
        };
        let out = match child.wait_with_output() {
            Ok(out) => out,
            Err(e) => {
                return Err(anyhow!(
                    "cannot wait for 'login' on the container engine: {e}"
                ))
            }
        };
        if !out.status.success() {
            return Err(anyhow!(
                "container engine 'login' failed for {registry}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        match written {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!(
                "cannot pass the token to the container engine: {e}"
            )),
        }
    }
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod tests;

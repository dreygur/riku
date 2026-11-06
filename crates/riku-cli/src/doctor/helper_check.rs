//! `riku doctor` check for the root router helper.
//!
//! Security: doctor runs as the deploy user and only asks the helper for its
//! `status`; the reply is data and is only printed.

use std::process::Command;

use anyhow::Result;
use riku_route::{Reply, Request};

use crate::config::RikuPaths;

use super::Check;

const NAME: &str = "router helper";
const UNIT: &str = "riku-router-helper.service";
const FIX: &str = "sudo systemctl enable --now riku-router-helper.service";

/// `Some(active)` on a systemd host, `None` when `systemctl` is missing.
fn unit_active() -> Option<bool> {
    if which::which("systemctl").is_err() {
        return None;
    }
    match Command::new("systemctl").args(["is-active", UNIT]).output() {
        Ok(output) => Some(output.status.success()),
        Err(_) => None,
    }
}

/// Turn the unit state and the helper's answer to `status` into a check.
/// `active` is `None` where there is no systemd to ask.
fn judge(active: Option<bool>, reply: Result<Reply>) -> Check {
    if active == Some(false) {
        return Check::fail(NAME, format!("{UNIT} is not active, run: {FIX}"));
    }
    match reply {
        Ok(Reply::Status { version }) => {
            Check::ok(NAME, format!("answers status, version {version}"))
        }
        Ok(other) => Check::fail(NAME, format!("unexpected answer to status: {other:?}")),
        Err(e) => Check::fail(NAME, format!("{e}. Run: {FIX}")),
    }
}

/// Ask the helper on its socket whether it is alive.
pub fn router_helper(paths: &RikuPaths) -> Check {
    let reply = crate::deploy::router::send_to_helper(paths, &Request::Status {});
    judge(unit_active(), reply)
}

#[cfg(test)]
#[path = "helper_check_tests.rs"]
mod tests;

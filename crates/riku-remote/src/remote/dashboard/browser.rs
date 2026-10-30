//! The laptop end of a dashboard session: the local port and the browser.
//!
//! Security model: the URL carries the session token as a query parameter.
//! It therefore appears in the argv of the local `open` or `xdg-open`
//! process for the moment that process runs, where another local user's
//! `ps` could see it, and the browser keeps it in its history. This is
//! accepted: both stay on this machine and the token dies with the session.

use std::net::TcpListener;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::util::display;

#[cfg(target_os = "macos")]
const OPENER: &str = "open";
#[cfg(not(target_os = "macos"))]
const OPENER: &str = "xdg-open";

/// `preferred` when nothing listens on it, otherwise any free port.
///
/// The port is free when checked, not reserved, so another program could
/// take it before ssh binds it. ssh then reports the failure.
pub(super) fn pick_local_port(preferred: u16) -> Result<u16> {
    if TcpListener::bind(("127.0.0.1", preferred)).is_ok() {
        return Ok(preferred);
    }
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("no free local port: {e}")),
    };
    match listener.local_addr() {
        Ok(addr) => Ok(addr.port()),
        Err(e) => Err(anyhow!("no free local port: {e}")),
    }
}

pub(super) fn dashboard_url(local_port: u16, token: &str) -> String {
    format!("http://127.0.0.1:{local_port}/?token={token}")
}

/// Opens `url` in the default browser, passing it, token included, as the
/// opener's argument. Failing to do so only warns, since the caller has
/// already printed the URL.
pub(super) fn open(url: &str) {
    let opened = Command::new(OPENER)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match opened {
        Ok(status) if status.success() => {}
        Ok(status) => display::warn(&format!("{OPENER} failed ({status}); open the URL above")),
        Err(e) => display::warn(&format!("could not run {OPENER}: {e}; open the URL above")),
    }
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;

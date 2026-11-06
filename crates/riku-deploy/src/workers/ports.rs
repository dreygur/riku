//! Which TCP port a web copy listens on: the one its app pins, or a free
//! one that no other copy of the app has.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;

const PORT_PICK_ATTEMPTS: usize = 20;

/// The `PORT` the app's ENV pins, if any. Some apps need a fixed port, such
/// as a compose file that publishes one.
pub(super) fn pinned_port(env: &HashMap<String, String>) -> Result<Option<u16>> {
    let Some(value) = env.get("PORT") else {
        return Ok(None);
    };
    match value.parse::<u16>() {
        Ok(port) if port != 0 => Ok(Some(port)),
        _ => Err(anyhow!("PORT in the app env is not a port: {value:?}")),
    }
}

pub(super) fn free_loopback_port() -> Result<u16> {
    match crate::util::get_free_port("127.0.0.1") {
        Ok(port) => Ok(port),
        Err(e) => Err(anyhow!("failed to allocate a port: {e}")),
    }
}

/// A port from `pick` that is not in `taken`. The OS hands out a port it
/// has just released again now and then, and two copies on one port leave
/// one of them crash looping behind the router.
#[allow(clippy::question_mark)]
pub(super) fn unused_port(taken: &[u16], mut pick: impl FnMut() -> Result<u16>) -> Result<u16> {
    for _ in 0..PORT_PICK_ATTEMPTS {
        let port = match pick() {
            Ok(port) => port,
            Err(e) => return Err(e),
        };
        if !taken.contains(&port) {
            return Ok(port);
        }
    }
    Err(anyhow!(
        "could not find a port distinct from the app's other copies"
    ))
}

/// Ports the app's enabled web copies run on. Old copies keep theirs while
/// their replacements start, so a new copy must not be given one of them.
pub(super) fn enabled_ports(app: &str, paths: &RikuPaths) -> Result<Vec<u16>> {
    match crate::router::web_copies(app, paths) {
        Ok(copies) => Ok(copies
            .iter()
            .filter_map(|copy| copy.env.get("PORT").and_then(|port| port.parse().ok()))
            .collect()),
        Err(e) => Err(anyhow!("cannot read the web copies of '{app}': {e}")),
    }
}

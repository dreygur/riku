//! After a reload, waiting for a port mode site's port to be listened on.
//!
//! `nginx -t` does not bind sockets, and a reload can report success when a
//! new `listen` fails, so a port another program holds would only show at
//! the next restart, taking every site down then. The helper instead checks
//! the port is in LISTEN and restores the previous files when it is not.

use std::collections::BTreeSet;
use std::thread;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(2);
const STEP: Duration = Duration::from_millis(100);

/// Wait up to two seconds for `probe` to list `port`.
pub(crate) fn wait_for_port(port: u16, probe: impl Fn() -> BTreeSet<u16>) -> Result<(), String> {
    let deadline = Instant::now() + WAIT;
    loop {
        if probe().contains(&port) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the router reloaded but is not listening on port {port}; another program may hold it"
            ));
        }
        thread::sleep(STEP);
    }
}

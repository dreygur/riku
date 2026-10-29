//! Ctrl-C, a closed terminal (SIGHUP) or SIGTERM during a dashboard session
//! set a flag instead of killing riku, so the session can restore the
//! terminal and stop its remote process and ssh master before it exits.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

const STOP_SIGNALS: [Signal; 3] = [Signal::SIGINT, Signal::SIGHUP, Signal::SIGTERM];

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_stop_signal(_: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Installs the handler for every stop signal. Without `SA_RESTART`, a
/// blocking read on the terminal returns `Interrupted`, so the password
/// prompt can give up.
pub(super) fn install() -> Result<()> {
    let action = SigAction::new(
        SigHandler::Handler(on_stop_signal),
        SaFlags::empty(),
        SigSet::empty(),
    );
    for signal in STOP_SIGNALS {
        // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
        if let Err(e) = unsafe { sigaction(signal, &action) } {
            return Err(anyhow!("could not install the {signal} handler: {e}"));
        }
    }
    Ok(())
}

pub(super) fn requested() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

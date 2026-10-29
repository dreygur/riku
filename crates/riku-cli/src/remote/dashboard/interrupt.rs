//! Ctrl-C during a dashboard session sets a flag instead of killing riku, so
//! the session can restore the terminal and stop its remote process and ssh
//! master before it exits.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Installs the SIGINT handler. Without `SA_RESTART`, a blocking read on the
/// terminal returns `Interrupted`, so the password prompt can give up.
pub(super) fn install() -> Result<()> {
    let action = SigAction::new(
        SigHandler::Handler(on_sigint),
        SaFlags::empty(),
        SigSet::empty(),
    );
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    match unsafe { sigaction(Signal::SIGINT, &action) } {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("could not install the Ctrl-C handler: {e}")),
    }
}

pub(super) fn requested() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

//! Supervisor daemon module for managing application processes.
//!
//! This module implements a process supervisor that replaces uWSGI Emperor,
//! handling process lifecycle, monitoring, and restart logic.

use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub mod config;
pub mod cron;
pub mod daemon;
pub mod health;
pub mod log_rotation;
pub mod process;
pub mod resource_limits;
pub mod stats;

pub use daemon::Supervisor;

// Shared atomics used by signal handlers and the main daemon loop.
// They live here (in the crate-level module) so that the `extern "C"` signal
// handlers, which must be in the same compilation unit as the statics they
// reference, can access them without any heap allocation or locking.
pub(crate) static RUNNING: AtomicBool = AtomicBool::new(true);
pub(crate) static RELOAD_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub(crate) static CONFIG_RELOAD_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Signal handler for graceful shutdown
pub fn setup_signal_handlers() -> Result<()> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, Signal};

        // SAFETY: Only async-signal-safe operations (atomic stores) are performed
        // inside these handlers. println!/eprintln! are NOT async-signal-safe and
        // must not be called here as they can deadlock if the signal interrupts a
        // write or allocation in the main thread. The main loop logs the event
        // after observing the flag change.
        extern "C" fn handle_sigterm(_: i32) {
            RUNNING.store(false, Ordering::SeqCst);
        }

        extern "C" fn handle_sigint(_: i32) {
            RUNNING.store(false, Ordering::SeqCst);
        }

        extern "C" fn handle_sighup(_: i32) {
            RELOAD_COUNTER.fetch_add(1, Ordering::SeqCst);
        }

        unsafe {
            sigaction(
                Signal::SIGTERM,
                &SigAction::new(
                    SigHandler::Handler(handle_sigterm),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
            sigaction(
                Signal::SIGINT,
                &SigAction::new(
                    SigHandler::Handler(handle_sigint),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
            sigaction(
                Signal::SIGHUP,
                &SigAction::new(
                    SigHandler::Handler(handle_sighup),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
        }
    }

    Ok(())
}

/// Check if the supervisor should continue running
pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

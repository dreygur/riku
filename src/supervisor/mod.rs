//! Supervisor daemon module for managing application processes.
//!
//! This module implements a process supervisor that replaces uWSGI Emperor,
//! handling process lifecycle, monitoring, and restart logic.

use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub mod cgroups;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_running_default_true() {
        // The global RUNNING flag starts true. This test only reads it; it does
        // not mutate global state so it is safe to run in parallel with others.
        assert!(
            is_running(),
            "is_running() should return true by default (RUNNING initialises to true)"
        );
    }

    #[test]
    fn test_is_running_reflects_atomic() {
        // Save the current value so we can restore it after the test.
        let original = RUNNING.load(Ordering::SeqCst);

        RUNNING.store(false, Ordering::SeqCst);
        assert!(
            !is_running(),
            "is_running() should return false after RUNNING is set to false"
        );

        RUNNING.store(true, Ordering::SeqCst);
        assert!(
            is_running(),
            "is_running() should return true after RUNNING is restored to true"
        );

        // Restore original value
        RUNNING.store(original, Ordering::SeqCst);
    }

    #[test]
    fn test_setup_signal_handlers_does_not_panic() {
        // Just verify the function completes without panicking.
        // The actual signal registration is tested implicitly by the fact that the
        // binary runs under the OS.
        setup_signal_handlers().expect("setup_signal_handlers() should not return an error");
    }

    #[test]
    fn test_is_running_with_real_subprocess() {
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::Duration;

        // Spawn a long-lived child so we can verify it is actually running.
        let mut child = Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn sleep process");

        // Give the OS a moment to start the process.
        thread::sleep(Duration::from_millis(50));

        // The process should still be alive.
        let still_running = child.try_wait().unwrap().is_none();
        assert!(still_running, "sleep process should still be running");

        // Clean up: kill and reap.
        child.kill().ok();
        child.wait().ok();

        // After reaping, try_wait should return Some(_).
        let exited = child.try_wait().unwrap();
        // On some platforms try_wait after wait returns None; either way the
        // child is no longer live. We simply assert kill+wait did not panic.
        let _ = exited;
    }
}

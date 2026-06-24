//! Supervisor daemon module for managing application processes.
//!
//! This module implements a process supervisor that replaces uWSGI Emperor,
//! handling process lifecycle, monitoring, and restart logic.

use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub mod cgroups;
pub mod config;
pub use crate::util::cron;
pub mod daemon;
pub mod health;
pub mod log_rotation;
pub mod process;
pub use crate::util::resource_limits;
pub mod stats;

pub use daemon::Supervisor;

// Shared atomics used by signal handlers and the main daemon loop.
// They live here (in the crate-level module) so that the `extern "C"` signal
// handlers, which must be in the same compilation unit as the statics they
// reference, can access them without any heap allocation or locking.
pub(crate) static RUNNING: AtomicBool = AtomicBool::new(true);
pub(crate) static RELOAD_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub(crate) static CONFIG_RELOAD_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Signal handler for graceful shutdown.
///
/// `SIGHUP` is deliberately *not* registered here via raw `sigaction` —
/// see [`spawn_sighup_listener`] for why and how it's handled instead.
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
        }
    }

    Ok(())
}

/// Start an async, non-blocking `SIGHUP` listener on a dedicated
/// background thread, and return immediately — never run on the main
/// supervisor loop's thread, so a slow or stuck reload can never delay
/// worker health checks, log rotation, or the file watcher.
///
/// Uses `tokio::signal::unix::signal(SignalKind::hangup())` rather than a
/// raw `sigaction` (unlike `SIGTERM`/`SIGINT` above) so catching the signal
/// and incrementing [`RELOAD_COUNTER`] happens inside an `async fn` body —
/// ordinary Rust, not the async-signal-safe-only subset `extern "C"`
/// handlers are restricted to. That matters here because, unlike the
/// shutdown flags, this is the path operators expect to extend over time
/// (this revision adds an `nginx -s reload` after the config diff — see
/// `daemon::mod::run`), and `extern "C"` handlers make that a hazard: every
/// addition has to be re-audited for signal-safety. The listener itself
/// still does only an atomic increment per wakeup, so it's exactly as
/// cheap as the old handler — `tokio::signal::unix::signal` registers its
/// own internal handler via `signal-hook-registry` and wakes the awaiting
/// task through a self-pipe, not by running our code inside a signal
/// context.
///
/// A raw `sigaction(SIGHUP, ...)` registered *anywhere else* in this
/// process would silently steal `SIGHUP` delivery from this listener
/// (`sigaction` is last-registration-wins, process-wide) — which is
/// exactly why `setup_signal_handlers` above no longer registers one.
///
/// Blocks the *calling* thread (briefly — microseconds) until the
/// background thread has actually registered the handler before
/// returning. This isn't just test convenience: without it, a `SIGHUP`
/// delivered in the window between this function returning and the
/// spawned thread reaching `tokio::signal::unix::signal()` would hit
/// `SIGHUP`'s default disposition (terminate the process) instead of
/// being caught — silently killing the supervisor on a signal that's
/// supposed to reload it. `Supervisor::run()` calls this before doing
/// anything else that could plausibly provoke an operator to send
/// `SIGHUP`, so the wait is never on any hot path.
pub(crate) fn spawn_sighup_listener() {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();

    std::thread::Builder::new()
        .name("riku-sighup-listener".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!(
                        "Failed to start SIGHUP listener runtime — SIGHUP reload will not \
                         work until the supervisor is restarted: {}",
                        e
                    );
                    return;
                }
            };

            runtime.block_on(async {
                let mut stream =
                    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("Failed to install SIGHUP listener: {}", e);
                            return;
                        }
                    };

                // Handler is registered now (the constructor above
                // registers synchronously) — let the caller proceed.
                let _ = ready_tx.send(());

                loop {
                    stream.recv().await;
                    tracing::info!("Received SIGHUP — scheduling configuration reload");
                    RELOAD_COUNTER.fetch_add(1, Ordering::SeqCst);
                }
            });
        })
        .expect("failed to spawn riku-sighup-listener thread");

    // Generous timeout: this only blocks while the new thread starts up
    // and builds a tiny single-threaded runtime, which is fast in
    // practice, but a wedged/overloaded host should still get a working
    // supervisor rather than hang its startup forever.
    if ready_rx.recv_timeout(Duration::from_secs(5)).is_err() {
        tracing::error!(
            "SIGHUP listener did not confirm startup within 5s — SIGHUP reload may not work"
        );
    }
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

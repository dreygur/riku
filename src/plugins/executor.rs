//! Plugin process execution helpers.
//!
//! Low-level utilities for spawning, timing out, and capturing output
//! from plugin child processes. Used exclusively by [`super::manager`].

use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

/// Read plugin timeout from `RIKU_PLUGIN_TIMEOUT` env var (seconds).
/// Defaults to 300 seconds (5 minutes).
pub(super) fn plugin_timeout() -> Duration {
    std::env::var("RIKU_PLUGIN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// Poll child every 200ms until it exits or the timeout elapses.
/// Kills the child (and reaps it) on timeout. Returns `true` if timed out.
pub(super) fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return false, // exited normally
            Ok(None) if start.elapsed() >= timeout => {
                child.kill().ok();
                child.wait().ok(); // reap to avoid zombie
                return true;
            }
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Emit captured stdout as INFO and stderr as WARN via tracing.
pub(super) fn emit_plugin_output(child: &mut std::process::Child, plugin_name: &str) {
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().flatten() {
            tracing::info!(plugin = plugin_name, "{}", line);
        }
    }
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().flatten() {
            tracing::warn!(plugin = plugin_name, "{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    // ── wait_with_timeout ────────────────────────────────────────────────────

    #[test]
    fn test_wait_with_timeout_fast_process_returns_false() {
        // A process that exits immediately must NOT be considered timed out.
        let mut child = Command::new("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn 'true'");

        let timed_out = wait_with_timeout(&mut child, Duration::from_secs(5));
        assert!(!timed_out, "fast-completing process should not time out");
    }

    #[test]
    fn test_wait_with_timeout_slow_process_returns_true() {
        // A process that takes 60 s must be killed when the timeout is 1 s.
        let mut child = Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn 'sleep 60'");

        let timed_out = wait_with_timeout(&mut child, Duration::from_secs(1));
        assert!(timed_out, "slow process should time out and be killed");
    }

    // ── emit_plugin_output ───────────────────────────────────────────────────

    #[test]
    fn test_emit_plugin_output_does_not_panic_with_output() {
        // Spawn a process that produces known stdout and stderr lines, then
        // verify emit_plugin_output drains both pipes without panicking.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("echo 'stdout line'; echo 'stderr line' >&2")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn sh");

        // Let the process finish before draining so all data is in the pipe.
        child.wait().ok();

        // Must not panic.
        emit_plugin_output(&mut child, "test-plugin");
    }

    #[test]
    fn test_emit_plugin_output_handles_no_pipes() {
        // When stdout/stderr are not piped, emit_plugin_output should be a
        // silent no-op (both take() calls return None).
        let mut child = Command::new("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn 'true'");

        child.wait().ok();
        emit_plugin_output(&mut child, "no-pipes-plugin");
    }

    // ── plugin_timeout ───────────────────────────────────────────────────────
    //
    // The three scenarios are in one sequential test to avoid data races on
    // the process-global `RIKU_PLUGIN_TIMEOUT` env var when tests run in
    // parallel.

    #[test]
    fn test_plugin_timeout_env_var_scenarios() {
        const KEY: &str = "RIKU_PLUGIN_TIMEOUT";

        // 1. Unset → default 300 s.
        std::env::remove_var(KEY);
        assert_eq!(
            plugin_timeout(),
            Duration::from_secs(300),
            "default plugin timeout should be 300 s"
        );

        // 2. Valid numeric value is respected.
        std::env::set_var(KEY, "42");
        assert_eq!(
            plugin_timeout(),
            Duration::from_secs(42),
            "plugin_timeout should honour RIKU_PLUGIN_TIMEOUT"
        );

        // 3. Non-numeric value falls back to default.
        std::env::set_var(KEY, "not-a-number");
        assert_eq!(
            plugin_timeout(),
            Duration::from_secs(300),
            "non-numeric RIKU_PLUGIN_TIMEOUT should fall back to 300 s"
        );

        // Clean up.
        std::env::remove_var(KEY);
    }
}

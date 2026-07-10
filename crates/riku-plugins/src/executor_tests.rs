use super::*;
use std::process::{Command, Stdio};

// ── classify_resource_exit ───────────────────────────────────────────────

#[test]
fn test_classify_resource_exit_detects_sigkill() {
    let status = Command::new("sh")
        .args(["-c", "kill -KILL $$"])
        .status()
        .unwrap();
    let cause = classify_resource_exit(&status, "");
    assert!(
        cause
            .as_deref()
            .map(|c| c.contains("SIGKILL"))
            .unwrap_or(false),
        "expected SIGKILL classification, got: {:?}",
        cause
    );
}

#[test]
fn test_classify_resource_exit_detects_sigxcpu() {
    let status = Command::new("sh")
        .args(["-c", "kill -XCPU $$"])
        .status()
        .unwrap();
    let cause = classify_resource_exit(&status, "");
    assert!(
        cause
            .as_deref()
            .map(|c| c.contains("SIGXCPU"))
            .unwrap_or(false),
        "expected SIGXCPU classification, got: {:?}",
        cause
    );
}

#[test]
fn test_classify_resource_exit_detects_allocator_failure_marker() {
    let status = Command::new("sh").args(["-c", "exit 2"]).status().unwrap();
    let cause = classify_resource_exit(&status, "xrealloc: cannot allocate 12345 bytes\n");
    assert!(
        cause
            .as_deref()
            .map(|c| c.contains("RLIMIT_AS"))
            .unwrap_or(false),
        "expected RLIMIT_AS classification from allocator marker, got: {:?}",
        cause
    );
}

#[test]
fn test_classify_resource_exit_ordinary_failure_returns_none() {
    // A plain non-zero exit with no signal and no allocator marker in
    // stderr is an ordinary application bug, not a resource limit —
    // must not be misclassified.
    let status = Command::new("sh").args(["-c", "exit 1"]).status().unwrap();
    assert_eq!(
        classify_resource_exit(&status, "some unrelated error message"),
        None
    );
}

// ── exit_code_for ────────────────────────────────────────────────────────

#[test]
fn test_exit_code_for_normal_exit() {
    let status = Command::new("sh").args(["-c", "exit 42"]).status().unwrap();
    assert_eq!(exit_code_for(&status), 42);
}

#[test]
fn test_exit_code_for_signal_uses_128_plus_signal_convention() {
    let status = Command::new("sh")
        .args(["-c", "kill -KILL $$"])
        .status()
        .unwrap();
    assert_eq!(exit_code_for(&status), 128 + 9);
}

// ── tee_output ───────────────────────────────────────────────────────────

#[test]
fn test_tee_output_captures_stderr_tail() {
    let mut child = Command::new("sh")
        .args(["-c", "echo to stdout; echo to stderr error >&2"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (handles, tail) = tee_output(&mut child);
    child.wait().unwrap();
    for h in handles {
        h.join().unwrap();
    }
    assert!(tail.lock().unwrap().contains("to stderr error"));
}

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

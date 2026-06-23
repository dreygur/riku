//! Plugin process execution helpers.
//!
//! Low-level utilities for spawning, timing out, and capturing output from
//! plugin child processes. Used by [`super::manager`] and [`super::runtime`].

use nix::sys::signal::{kill, killpg, Signal};
use nix::unistd::{getpgid, Pid};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Read plugin timeout from `RIKU_PLUGIN_TIMEOUT` env var (seconds).
/// Defaults to 300 seconds (5 minutes).
pub(crate) fn plugin_timeout() -> Duration {
    std::env::var("RIKU_PLUGIN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// `cmd.spawn()`, retrying on `ETXTBSY`.
///
/// `ETXTBSY` ("text file busy") is normally permanent — the loader refuses
/// to `execve()` a file that's genuinely still open for writing. But riku
/// routinely spawns scripts it (or `riku install-plugins`) only *just*
/// finished writing — runtime/hook plugins here, and any executable a
/// build/install step drops moments before something execs it. On Linux,
/// `execve()` of a file that was written-then-`rename()`d into place only
/// microseconds earlier can transiently return `ETXTBSY` even though no
/// writer is left: it's a known kernel race in the exec path's "deny
/// write" bookkeeping when many threads of the same process are
/// fork()ing/exec()ing concurrently (exactly riku's test suite, and exactly
/// a busy production host running many worker/hook spawns at once). The
/// condition self-resolves in microseconds, so a few retries with a short
/// backoff turns a spurious, permanent-looking failure into the success it
/// actually is — without masking a real, persistent `ETXTBSY` (e.g. an
/// actual concurrent writer), which will still fail after the retry budget
/// is exhausted.
pub(crate) fn spawn_retrying_etxtbsy(cmd: &mut Command) -> std::io::Result<Child> {
    const MAX_ATTEMPTS: u32 = 5;
    const INITIAL_BACKOFF: Duration = Duration::from_millis(5);

    let mut backoff = INITIAL_BACKOFF;
    for attempt in 1..=MAX_ATTEMPTS {
        match cmd.spawn() {
            Ok(child) => return Ok(child),
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && attempt < MAX_ATTEMPTS => {
                std::thread::sleep(backoff);
                backoff *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("loop always returns on its last iteration");
}

/// Cap on how much trailing stderr `tee_output` retains for post-mortem
/// classification. Just needs to be big enough to catch a one-line
/// allocator failure message ("xrealloc: cannot allocate N bytes"), not a
/// full build log — this isn't meant to replace the live-streamed output.
const STDERR_TAIL_CAP: usize = 4096;

/// Spawn background threads that mirror `child`'s stdout/stderr to this
/// process's own stdout/stderr line-by-line — preserving real-time
/// streaming for whoever's watching `riku deploy` — while also retaining
/// the last [`STDERR_TAIL_CAP`] bytes of stderr in the returned buffer, so
/// a failed exit can be classified by [`classify_resource_exit`] afterward.
/// `child.stdout`/`child.stderr` must be `Stdio::piped()` for this to do
/// anything (a `None` pipe is silently skipped).
///
/// Callers must `child.wait()` (or `wait_with_timeout`) and then join the
/// returned handles before reading the tail buffer, so they don't race the
/// reader threads still draining the pipes.
pub(crate) fn tee_output(child: &mut Child) -> (Vec<JoinHandle<()>>, Arc<Mutex<String>>) {
    let tail = Arc::new(Mutex::new(String::new()));
    let mut handles = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                println!("{}", line);
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let tail = Arc::clone(&tail);
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("{}", line);
                let mut buf = tail.lock().unwrap();
                buf.push_str(&line);
                buf.push('\n');
                if buf.len() > STDERR_TAIL_CAP {
                    let excess = buf.len() - STDERR_TAIL_CAP;
                    buf.drain(..excess);
                }
            }
        }));
    }

    (handles, tail)
}

/// Classify a finished child's exit as resource exhaustion, distinguishing
/// it from an ordinary application failure so callers don't misreport
/// unrelated bugs as resource limits. Two cases:
///
/// - **Killed directly by the kernel**: `SIGKILL` (the OOM killer, or a
///   cgroup `memory.max` limit) or `SIGXCPU` (`RLIMIT_CPU` exceeded). These
///   show up as a signal on the exit status, not an exit code.
/// - **Its own allocator gave up**: hitting `RLIMIT_AS` doesn't kill the
///   process — `malloc`/`mmap` just starts returning `ENOMEM`, which most
///   allocators (glibc, bash's `xrealloc`) report on stderr and then exit
///   non-zero on their own. Detected via a substring match on the
///   `tee_output`-captured stderr tail.
///
/// Returns `None` when neither pattern matches.
pub(crate) fn classify_resource_exit(status: &ExitStatus, stderr_tail: &str) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(9) => {
                return Some(
                    "killed by SIGKILL — the kernel's OOM killer or a cgroup memory.max limit \
                     terminated it directly"
                        .to_string(),
                )
            }
            Some(24) => {
                return Some(
                    "killed by SIGXCPU — exceeded its configured CPU time limit (RLIMIT_CPU)"
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    const ALLOCATOR_FAILURE_MARKERS: &[&str] = &[
        "cannot allocate memory",
        "out of memory",
        "xrealloc:",
        "xmalloc:",
        "memory exhausted",
    ];
    let lower = stderr_tail.to_lowercase();
    ALLOCATOR_FAILURE_MARKERS
        .iter()
        .find(|marker| lower.contains(*marker))
        .map(|marker| {
            format!(
                "its own allocator reported '{}' — it hit the configured memory ceiling \
                 (RLIMIT_AS) before the kernel needed to step in",
                marker
            )
        })
}

/// The shell-convention exit code for a finished child: its real exit code
/// if it has one, or `128 + signal` if it was killed by a signal (the same
/// convention `sh`/`bash` use), or `1` if neither is available.
pub(crate) fn exit_code_for(status: &ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

/// Poll child every 200ms until it exits or the timeout elapses.
/// Kills the child (and reaps it) on timeout. Returns `true` if timed out.
pub(crate) fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return false, // exited normally
            Ok(None) if start.elapsed() >= timeout => {
                terminate_process_tree(child.id());
                child.wait().ok(); // reap to avoid zombie
                return true;
            }
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Kill `pid`, and the entire process group if `pid` leads its own group
/// (i.e. the caller spawned it with `process_group(0)`). Plugins and
/// lifecycle hooks are arbitrary shell scripts; one that backgrounds work
/// (`make -j &`, a daemonizing build step) spawns grandchildren outside the
/// single-PID kill that `Child::kill()` sends, leaving them as orphans once
/// the timeout fires. Falls back to a plain `kill` if `pid` isn't a group
/// leader, so this is safe even for callers that didn't set up a dedicated
/// group.
fn terminate_process_tree(pid: u32) {
    let pid = Pid::from_raw(pid as i32);
    match getpgid(Some(pid)) {
        Ok(pgid) if pgid == pid => {
            let _ = killpg(pid, Signal::SIGKILL);
        }
        _ => {
            let _ = kill(pid, Signal::SIGKILL);
        }
    }
}

/// Emit captured stdout as INFO and stderr as WARN via tracing.
pub(super) fn emit_plugin_output(child: &mut std::process::Child, plugin_name: &str) {
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            tracing::info!(plugin = plugin_name, "{}", line);
        }
    }
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            tracing::warn!(plugin = plugin_name, "{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
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
}

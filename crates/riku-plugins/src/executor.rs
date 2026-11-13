//! Plugin process execution helpers.
//!
//! Low-level utilities for spawning, timing out, and capturing output from
//! plugin child processes. Used by [`super::manager`] and [`super::runtime`].

use nix::sys::signal::{kill, killpg, Signal};
use nix::unistd::{getpgid, Pid};
use std::io::{BufReader, Read};
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::util::display::{self, for_each_line, OutputTail};

/// Read plugin timeout from `RIKU_PLUGIN_TIMEOUT` env var (seconds).
/// Defaults to 300 seconds (5 minutes).
pub fn plugin_timeout() -> Duration {
    timeout_from_raw(std::env::var("RIKU_PLUGIN_TIMEOUT").ok())
}

/// Parsing half of [`plugin_timeout`], split out so it can be tested without
/// writing to the process-global environment that every parallel test shares.
/// An unset or unparsable value means the default rather than an error: a
/// typo in an operator's env file must not stop deployments.
fn timeout_from_raw(raw: Option<String>) -> Duration {
    raw.and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

pub use riku_util::spawn::{output_retrying_etxtbsy, spawn_retrying_etxtbsy};

/// Cap on how much trailing stderr `tee_output` retains for post-mortem
/// classification. Just needs to be big enough to catch a one-line
/// allocator failure message ("xrealloc: cannot allocate N bytes"), not a
/// full build log: this isn't meant to replace the live-streamed output.
const STDERR_TAIL_CAP: usize = 4096;

/// Keeps the last `STDERR_TAIL_CAP` bytes of `buf`, cutting on a character
/// boundary so multi-byte text never splits.
fn trim_tail(buf: &mut String) {
    if buf.len() <= STDERR_TAIL_CAP {
        return;
    }
    let mut cut = buf.len() - STDERR_TAIL_CAP;
    while !buf.is_char_boundary(cut) {
        cut += 1;
    }
    buf.drain(..cut);
}

/// Spawn background threads that mirror `child`'s stdout and stderr to this
/// process's stdout line by line, each under the output rail (`│  line`), so
/// whoever watches `riku deploy` or `git push` sees it live. Both go to
/// stdout because the rest of the deploy rail does: over ssh, stdout and
/// stderr arrive on separate channels and lose their order. Every line from
/// either stream also goes into `recent`. The last [`STDERR_TAIL_CAP`] bytes of stderr are kept
/// in the returned buffer, so a failed exit can be classified by
/// [`classify_resource_exit`] afterward. `child.stdout`/`child.stderr` must
/// be `Stdio::piped()` for this to do anything (a `None` pipe is silently
/// skipped).
///
/// Callers must `child.wait()` (or `wait_with_timeout`) and then join the
/// returned handles before reading the tail buffer, so they don't race the
/// reader threads still draining the pipes.
pub fn tee_output(
    child: &mut Child,
    recent: &OutputTail,
) -> (Vec<JoinHandle<()>>, Arc<Mutex<String>>) {
    let tail = Arc::new(Mutex::new(String::new()));
    let mut handles = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        let recent = recent.clone();
        handles.push(std::thread::spawn(move || {
            for_each_line(stdout, |line| {
                display::detail(line);
                recent.push(line);
            });
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let tail = Arc::clone(&tail);
        let recent = recent.clone();
        handles.push(std::thread::spawn(move || {
            for_each_line(stderr, |line| {
                display::detail(line);
                recent.push(line);
                let mut buf = match tail.lock() {
                    Ok(buf) => buf,
                    Err(poisoned) => poisoned.into_inner(),
                };
                buf.push_str(line);
                buf.push('\n');
                trim_tail(&mut buf);
            });
        }));
    }

    (handles, tail)
}

/// Spawn a thread that captures `child`'s entire stdout into a buffer.
///
/// For seams where stdout itself is the return value (an addon verb's
/// response payload), as opposed to [`tee_output`], which mirrors
/// stdout/stderr live and only retains a diagnostic stderr tail. Returns the
/// join handle (`None` if `child.stdout` wasn't piped) and the buffer;
/// callers must join the handle (after `wait`/`wait_with_timeout`) before
/// reading the buffer, so they don't race the reader thread.
pub fn capture_stdout(child: &mut Child) -> (Option<JoinHandle<()>>, Arc<Mutex<String>>) {
    let buf = Arc::new(Mutex::new(String::new()));
    let handle = child.stdout.take().map(|out| {
        let buf = Arc::clone(&buf);
        std::thread::spawn(move || {
            let mut s = String::new();
            if BufReader::new(out).read_to_string(&mut s).is_ok() {
                *buf.lock().unwrap() = s;
            }
        })
    });
    (handle, buf)
}

/// Spawn a thread that streams `child`'s stderr line-by-line to `on_line`,
/// typically a `tracing::info!` call with seam-specific target and fields,
/// which is why this takes a callback rather than logging itself. Returns
/// the join handle (`None` if `child.stderr` wasn't piped); callers must
/// join it (after `wait`/`wait_with_timeout`) before treating the child as
/// done, so no stderr output is lost.
pub fn stream_stderr(
    child: &mut Child,
    on_line: impl Fn(&str) + Send + 'static,
) -> Option<JoinHandle<()>> {
    child
        .stderr
        .take()
        .map(|err| std::thread::spawn(move || for_each_line(err, on_line)))
}

/// Classify a finished child's exit as resource exhaustion, distinguishing
/// it from an ordinary application failure so callers don't misreport
/// unrelated bugs as resource limits. Two cases:
///
/// - **Killed directly by the kernel**: `SIGKILL` (the OOM killer, or a
///   cgroup `memory.max` limit) or `SIGXCPU` (`RLIMIT_CPU` exceeded). These
///   show up as a signal on the exit status, not an exit code.
/// - **Its own allocator gave up**: hitting `RLIMIT_AS` doesn't kill the
///   process: `malloc`/`mmap` just starts returning `ENOMEM`, which most
///   allocators (glibc, bash's `xrealloc`) report on stderr and then exit
///   non-zero on their own. Detected via a substring match on the
///   `tee_output`-captured stderr tail.
///
/// Returns `None` when neither pattern matches.
pub fn classify_resource_exit(status: &ExitStatus, stderr_tail: &str) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(9) => {
                return Some(
                    "killed by SIGKILL: the kernel's OOM killer or a cgroup memory.max limit \
                     terminated it directly"
                        .to_string(),
                )
            }
            Some(24) => {
                return Some(
                    "killed by SIGXCPU: exceeded its configured CPU time limit (RLIMIT_CPU)"
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
                "its own allocator reported '{}': it hit the configured memory ceiling \
                 (RLIMIT_AS) before the kernel needed to step in",
                marker
            )
        })
}

/// The shell-convention exit code for a finished child: its real exit code
/// if it has one, or `128 + signal` if it was killed by a signal (the same
/// convention `sh`/`bash` use), or `1` if neither is available.
pub fn exit_code_for(status: &ExitStatus) -> i32 {
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
pub fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> bool {
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
        for_each_line(stdout, |line| {
            tracing::info!(plugin = plugin_name, "{}", line)
        });
    }
    if let Some(stderr) = child.stderr.take() {
        for_each_line(stderr, |line| {
            tracing::warn!(plugin = plugin_name, "{}", line)
        });
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;

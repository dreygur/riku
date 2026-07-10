//! Plugin process execution helpers.
//!
//! Low-level utilities for spawning, timing out, and capturing output from
//! plugin child processes. Used by [`super::manager`] and [`super::runtime`].

use nix::sys::signal::{kill, killpg, Signal};
use nix::unistd::{getpgid, Pid};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Read plugin timeout from `RIKU_PLUGIN_TIMEOUT` env var (seconds).
/// Defaults to 300 seconds (5 minutes).
pub fn plugin_timeout() -> Duration {
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
pub fn spawn_retrying_etxtbsy(cmd: &mut Command) -> std::io::Result<Child> {
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
pub fn tee_output(child: &mut Child) -> (Vec<JoinHandle<()>>, Arc<Mutex<String>>) {
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

/// Spawn a thread that captures `child`'s entire stdout into a buffer.
///
/// For seams where stdout itself is the return value (a verb/filter/panel's
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

/// Spawn a thread that streams `child`'s stderr line-by-line to `on_line` —
/// typically a `tracing::info!` call with seam-specific target and fields,
/// which is why this takes a callback rather than logging itself. Returns
/// the join handle (`None` if `child.stderr` wasn't piped); callers must
/// join it (after `wait`/`wait_with_timeout`) before treating the child as
/// done, so no stderr output is lost.
pub fn stream_stderr(
    child: &mut Child,
    on_line: impl Fn(&str) + Send + 'static,
) -> Option<JoinHandle<()>> {
    child.stderr.take().map(|err| {
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                on_line(&line);
            }
        })
    })
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
pub fn classify_resource_exit(status: &ExitStatus, stderr_tail: &str) -> Option<String> {
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
#[path = "executor_tests.rs"]
mod tests;

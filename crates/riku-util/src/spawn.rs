//! Process spawning that tolerates the transient `ETXTBSY` race.

use std::process::{Child, Command, Output};
use std::time::Duration;

/// `cmd.spawn()`, retrying on `ETXTBSY`.
///
/// `ETXTBSY` ("text file busy") is normally permanent, the loader refuses
/// to `execve()` a file that's genuinely still open for writing. But riku
/// routinely spawns scripts it (or `riku install-plugins`) only *just*
/// finished writing: runtime/hook plugins here, and any executable a
/// build/install step drops moments before something execs it. On Linux,
/// `execve()` of a file that was written-then-`rename()`d into place only
/// microseconds earlier can transiently return `ETXTBSY` even though no
/// writer is left: it's a known kernel race in the exec path's "deny
/// write" bookkeeping when many threads of the same process are
/// fork()ing/exec()ing concurrently (exactly riku's test suite, and exactly
/// a busy production host running many worker/hook spawns at once). The
/// condition self-resolves in microseconds, so a few retries with a short
/// backoff turns a spurious, permanent-looking failure into the success it
/// actually is: without masking a real, persistent `ETXTBSY` (e.g. an
/// actual concurrent writer), which will still fail after the retry budget
/// is exhausted.
pub fn spawn_retrying_etxtbsy(cmd: &mut Command) -> std::io::Result<Child> {
    retry_etxtbsy(|| cmd.spawn())
}

/// `cmd.output()`, retrying on `ETXTBSY` for the same reason as
/// [`spawn_retrying_etxtbsy`]. Use this for the plugin subcommands whose
/// stdout riku consumes (`env`, `start`): they run moments after a build or
/// install step may have written the plugin file, so a plain `output()` can
/// hit the same transient "text file busy" that the spawn path guards
/// against.
pub fn output_retrying_etxtbsy(cmd: &mut Command) -> std::io::Result<Output> {
    retry_etxtbsy(|| cmd.output())
}

fn retry_etxtbsy<T>(mut attempt: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    const MAX_ATTEMPTS: u32 = 5;
    const INITIAL_BACKOFF: Duration = Duration::from_millis(5);

    let mut backoff = INITIAL_BACKOFF;
    for n in 1..=MAX_ATTEMPTS {
        match attempt() {
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && n < MAX_ATTEMPTS => {
                std::thread::sleep(backoff);
                backoff *= 2;
            }
            other => return other,
        }
    }
    unreachable!("loop always returns on its last iteration");
}

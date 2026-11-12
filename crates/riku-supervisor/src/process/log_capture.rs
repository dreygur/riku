//! Copying a worker's stdout and stderr into its log file, across external
//! log rotation.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::util::log_stamp::stamp_line;

/// How often (at most) to `stat()` the log path to check for external
/// rotation, throttled by wall-clock time rather than line count so the
/// check cost stays constant regardless of how chatty the worker's stdout
/// is.
const ROTATION_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Read `reader` line by line, appending each line to `file` (opened on
/// `log_path`), staying correct across two different kinds of external log
/// rotation:
///
/// - **copytruncate** (same inode, file truncated to 0 in place): already
///   handled for free: `file` was opened with `O_APPEND`, so the kernel
///   seeks to the file's *current* end before every `write()`, which after
///   a truncate is simply offset 0. No detection needed, no hole, no panic.
/// - **rename + recreate** (the conventional `logrotate` default): the path
///   now refers to a *different* inode than the one `file` has open, `file`
///   would otherwise keep appending into the renamed-away copy, invisible
///   to anything tailing the original path. Detected here by periodically
///   comparing `fstat(file)` against `stat(log_path)`; on a mismatch, the
///   file is reopened at `log_path` (in append mode) and capture continues
///   through the new handle, with nothing more than a log line lost in the
///   window between rotation and the next check.
///
/// Never panics: every fallible step (write, flush, reopen, the rotation
/// check's own stat calls) degrades to "log this line was dropped" rather
/// than crashing the thread, since losing a log line is recoverable and
/// killing the capture thread silently is not.
/// If `log_path` now refers to a different inode than `file` has open,
/// reopen it in append mode and swap `*file` for the new handle. A no-op
/// when nothing has rotated, when the path is temporarily missing
/// (deleted but not yet recreated: kept writing through the existing,
/// still-valid-just-unlinked fd until the next check), or when reopening
/// itself fails (logged, old handle kept so lines keep landing somewhere
/// rather than being dropped entirely).
fn reopen_if_rotated(log_path: &std::path::Path, file: &mut File, stream_name: &str) {
    use std::os::unix::fs::MetadataExt;

    let fd_inode = file.metadata().ok().map(|m| (m.dev(), m.ino()));
    let path_inode = match fs::metadata(log_path) {
        Ok(m) => (m.dev(), m.ino()),
        Err(_) => return,
    };

    if fd_inode == Some(path_inode) {
        return;
    }

    let _ = file.flush();
    match OpenOptions::new().create(true).append(true).open(log_path) {
        Ok(reopened) => {
            tracing::info!(
                "{} log file rotated externally, reopened: {}",
                stream_name,
                log_path.display()
            );
            *file = reopened;
        }
        Err(e) => {
            tracing::warn!(
                "Failed to reopen rotated {} log {}: {}, continuing to write to the old file handle",
                stream_name,
                log_path.display(),
                e
            );
        }
    }
}

/// Read `reader` line by line, appending each line to `file` (opened on
/// `log_path`) behind its capture time (`riku_util::log_stamp`), staying
/// correct across two different kinds of external log rotation:
///
/// - **copytruncate** (same inode, file truncated to 0 in place): already
///   handled for free: `file` was opened with `O_APPEND`, so the kernel
///   seeks to the file's *current* end before every `write()`, which after
///   a truncate is simply offset 0. No detection needed, no hole, no panic.
/// - **rename + recreate** (the conventional `logrotate` default): the path
///   now refers to a *different* inode than the one `file` has open,
///   detected by [`reopen_if_rotated`], called at most once per
///   [`ROTATION_CHECK_INTERVAL`] so the check cost stays constant
///   regardless of log volume.
///
/// Never panics: every fallible step (write, flush, reopen) degrades to
/// "this line was dropped" rather than crashing the thread, since losing a
/// log line is recoverable and killing the capture thread silently is not.
pub(super) fn run_log_capture_thread(
    reader: impl std::io::Read,
    log_path: &std::path::Path,
    mut file: File,
    stream_name: &'static str,
) {
    let mut last_rotation_check = Instant::now();
    let reader = BufReader::new(reader);

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                tracing::debug!("Error reading {}: {}", stream_name, e);
                break;
            }
        };

        if last_rotation_check.elapsed() >= ROTATION_CHECK_INTERVAL {
            last_rotation_check = Instant::now();
            reopen_if_rotated(log_path, &mut file, stream_name);
        }

        let _ = writeln!(file, "{}", stamp_line(Utc::now(), &line));
        let _ = file.flush();
    }

    drop(file);
    tracing::debug!("{} log capture thread exited", stream_name);
}

#[cfg(test)]
#[path = "log_capture_tests.rs"]
mod tests;

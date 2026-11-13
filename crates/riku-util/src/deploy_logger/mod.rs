//! Deploy logger: writes timestamped entries to an app's deploy log while
//! the rail prints them for git push streaming.
//!
//! The log holds the last [`KEEP_DEPLOYS`] deploys, each starting with a
//! [`DEPLOY_MARK`] line, and every line streamed on the rail (build,
//! release and runtime plugin output, through [`DeployLogSink`]) besides
//! the steps. Older deploys are trimmed when a deploy opens the log, and a
//! deploy stops copying streamed lines once it wrote [`DEPLOY_LOG_BYTES`],
//! so the file stays under about twice that.
//!
//! ## Security Model
//!
//! The log file is created at a path determined by `RikuPaths::deploy_log_file`,
//! which is always within `{riku_root}/logs/{app}/`. App names are validated
//! before reaching this module, so no path traversal is possible. Build
//! output can carry what an app prints of its secrets, which the rail shows
//! only to the pusher, so the file is `0600`.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use chrono::Local;

use crate::display;
use crate::fs::write_private;

mod trim;

use trim::recent_deploys;

/// How many deploys the log keeps, the one starting included.
pub const KEEP_DEPLOYS: usize = 10;
/// The byte budget of the kept history, and of one deploy's own lines.
pub const DEPLOY_LOG_BYTES: usize = 5 * 1024 * 1024;
/// The text of the line that starts each deploy in the log.
pub const DEPLOY_MARK: &str = "---- deploy ----";
/// Written once when a deploy's lines reach the byte budget.
pub const TRUNCATED: &str = "[output truncated: this deploy reached the deploy log's size limit]";

const LOG_MODE: u32 = 0o600;

/// The limits a log is opened with. [`DeployLogger::new`] uses
/// [`KEEP_DEPLOYS`] and [`DEPLOY_LOG_BYTES`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct LogLimits {
    pub(crate) deploys: usize,
    pub(crate) bytes: usize,
}

struct LogFile {
    file: File,
    written: usize,
    budget: usize,
    truncated: bool,
}

impl LogFile {
    fn write_line(&mut self, line: &str) {
        let _ = writeln!(self.file, "{line}");
    }

    /// A line of command output, dropped once this deploy's output used up
    /// its budget. Steps, warnings and the outcome are always written.
    fn write_output(&mut self, line: &str) {
        if self.truncated {
            return;
        }
        if self.written + line.len() > self.budget {
            self.truncated = true;
            self.write_line(TRUNCATED);
            return;
        }
        self.written += line.len() + 1;
        self.write_line(line);
    }
}

fn stamped(msg: &str) -> String {
    format!("[{}] {msg}", Local::now().format("%Y-%m-%dT%H:%M:%S%z"))
}

/// A handle on the open deploy log that the threads reading a command's
/// output share; see [`display::OutputTail::logged_to`].
#[derive(Clone)]
pub struct DeployLogSink {
    log: Arc<Mutex<LogFile>>,
}

impl DeployLogSink {
    fn locked(&self) -> std::sync::MutexGuard<'_, LogFile> {
        match self.log.lock() {
            Ok(log) => log,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn write(&self, line: &str) {
        self.locked().write_line(line);
    }

    /// One line of a command's output, stamped and set off with `| `.
    pub fn write_streamed(&self, line: &str) {
        self.locked().write_output(&stamped(&format!("| {line}")));
    }
}

/// A deploy logger that writes timestamped entries to a log file
/// while also printing them to stdout (for git push streaming).
pub struct DeployLogger {
    sink: DeployLogSink,
}

/// Rewrite `path` to the deploys `limits` keep, the one about to start
/// not counted yet. A missing file is fine.
fn trim_history(path: &Path, limits: LogLimits) -> Result<()> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    let kept = recent_deploys(&text, limits.deploys.saturating_sub(1), limits.bytes);
    if kept.len() == text.len() {
        return Ok(());
    }
    write_private(path, kept.as_bytes())
}

impl DeployLogger {
    /// Open the deploy log for a new deploy: trim it to the last
    /// [`KEEP_DEPLOYS`] deploys and [`DEPLOY_LOG_BYTES`], then append this
    /// deploy's start line.
    pub fn new(log_path: &Path) -> Result<Self> {
        Self::open(
            log_path,
            LogLimits {
                deploys: KEEP_DEPLOYS,
                bytes: DEPLOY_LOG_BYTES,
            },
        )
    }

    #[allow(clippy::question_mark)]
    pub(crate) fn open(log_path: &Path, limits: LogLimits) -> Result<Self> {
        if let Some(parent) = log_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(anyhow!("cannot create {}: {e}", parent.display()));
            }
        }
        if let Err(e) = trim_history(log_path, limits) {
            return Err(e);
        }
        let file = match OpenOptions::new()
            .create(true)
            .append(true)
            .mode(LOG_MODE)
            .open(log_path)
        {
            Ok(file) => file,
            Err(e) => return Err(anyhow!("cannot open {}: {e}", log_path.display())),
        };
        if let Err(e) = file.set_permissions(fs::Permissions::from_mode(LOG_MODE)) {
            return Err(anyhow!("cannot chmod {}: {e}", log_path.display()));
        }
        let sink = DeployLogSink {
            log: Arc::new(Mutex::new(LogFile {
                file,
                written: 0,
                budget: limits.bytes,
                truncated: false,
            })),
        };
        sink.write(&stamped(DEPLOY_MARK));
        Ok(Self { sink })
    }

    /// The handle a command's output tail writes through.
    pub fn sink(&self) -> DeployLogSink {
        self.sink.clone()
    }

    /// Print a finished step (`◇  label  detail`) and log it. The terminal
    /// output streams back to the pusher through git's `remote:` lines.
    pub fn step(&mut self, label: &str, detail: &str) {
        display::step_done(label, detail);
        self.log_raw(&format!("{label} {detail}"));
    }

    /// Print a detail line under the current step and log it.
    pub fn detail(&mut self, msg: &str) {
        display::detail(msg);
        self.log_raw(msg);
    }

    /// Write a warning to stderr and the log file.
    pub fn log_warn(&mut self, msg: &str) {
        display::warn(msg);
        self.sink.write(&format!("[WARN] {msg}"));
    }

    /// Write an error to stderr and the log file.
    pub fn log_error(&mut self, msg: &str) {
        display::error(msg);
        self.sink.write(&format!("[ERROR] {msg}"));
    }

    /// Write a raw line (no prefix) directly to the log file only.
    ///
    /// Used for recording metadata (e.g. deploy start/end markers) that
    /// should appear in the log but not echo to the user's terminal.
    pub fn log_raw(&mut self, msg: &str) {
        self.sink.write(&stamped(msg));
    }
}

#[cfg(test)]
mod tests;

//! Deploy logger: writes timestamped entries to a deploy log file
//! while also printing to stdout for git push streaming.
//!
//! ## Security Model
//!
//! The log file is created at a path determined by `RikuPaths::deploy_log_file`,
//! which is always within `{riku_root}/logs/{app}/`. App names are validated
//! before reaching this module, so no path traversal is possible.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

/// A deploy logger that writes timestamped entries to a log file
/// while also printing them to stdout (for git push streaming).
pub struct DeployLogger {
    log_file: File,
}

impl DeployLogger {
    /// Open (or create and truncate) the deploy log file for this deployment.
    ///
    /// Each new deploy overwrites the previous log so the file always reflects
    /// the most recent deployment attempt.
    pub fn new(log_path: &PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path)?;
        Ok(Self { log_file })
    }

    /// Write an informational line to both stdout and the log file.
    ///
    /// stdout output uses the Heroku-style `"-----> "` prefix so it streams
    /// back to the user's terminal via git remote output.
    pub fn log(&mut self, msg: &str) {
        println!("-----> {}", msg);
        let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S%z");
        let _ = writeln!(self.log_file, "[{}] {}", timestamp, msg);
    }

    /// Write a warning to stderr and the log file.
    pub fn log_warn(&mut self, msg: &str) {
        eprintln!(" !     {}", msg);
        let _ = writeln!(self.log_file, "[WARN] {}", msg);
    }

    /// Write an error to stderr and the log file.
    pub fn log_error(&mut self, msg: &str) {
        eprintln!(" !     {}", msg);
        let _ = writeln!(self.log_file, "[ERROR] {}", msg);
    }

    /// Write a raw line (no prefix) directly to the log file only.
    ///
    /// Used for recording metadata (e.g. deploy start/end markers) that
    /// should appear in the log but not echo to the user's terminal.
    pub fn log_raw(&mut self, msg: &str) {
        let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S%z");
        let _ = writeln!(self.log_file, "[{}] {}", timestamp, msg);
    }
}

//! Log rotation module for the supervisor.
//!
//! Handles automatic log file rotation based on size and retention policies.

use anyhow::Result;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(test)]
mod tests;

/// Log rotation configuration.
#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    /// Maximum log file size in bytes before rotation (default: 10MB)
    pub max_size: u64,
    /// Number of rotated logs to keep (default: 5)
    pub retention_count: u32,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        LogRotationConfig {
            max_size: 10 * 1024 * 1024, // 10MB
            retention_count: 5,
        }
    }
}

/// Log rotator that manages log file rotation.
pub struct LogRotator {
    config: LogRotationConfig,
}

impl LogRotator {
    /// Create a new log rotator with the given configuration.
    pub fn new(config: LogRotationConfig) -> Self {
        LogRotator { config }
    }

    /// Create a log rotator with default configuration.
    #[cfg(test)]
    pub fn with_defaults() -> Self {
        LogRotator {
            config: LogRotationConfig::default(),
        }
    }

    /// Check if a log file needs rotation.
    pub fn needs_rotation(&self, log_path: &Path) -> Result<bool> {
        if !log_path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(log_path)?;
        Ok(metadata.len() >= self.config.max_size)
    }

    /// Rotate a log file.
    ///
    /// Rotation process:
    /// 1. Rename current log to log.1
    /// 2. Shift existing rotated logs (log.1 -> log.2, etc.)
    /// 3. Delete oldest logs beyond retention count
    /// 4. Create new empty log file
    pub fn rotate(&self, log_path: &Path) -> Result<()> {
        if !log_path.exists() {
            return Ok(());
        }

        let log_dir = log_path.parent().unwrap_or(Path::new("."));
        let log_name = log_path.file_name().unwrap_or_default().to_string_lossy();

        // Delete oldest log beyond retention
        let oldest_path = log_dir.join(format!("{}.{}", log_name, self.config.retention_count));
        if oldest_path.exists() {
            let _ = fs::remove_file(&oldest_path);
        }

        // Shift existing rotated logs
        for i in (1..self.config.retention_count).rev() {
            let old_path = log_dir.join(format!("{}.{}", log_name, i));
            let new_path = log_dir.join(format!("{}.{}", log_name, i + 1));
            if old_path.exists() {
                let _ = fs::rename(&old_path, &new_path);
            }
        }

        // Rotate current log by streaming (avoids reading the whole file into RAM).
        let rotated_path = log_dir.join(format!("{}.1", log_name));
        let mut src = File::open(log_path)?;
        let mut dst = File::create(&rotated_path)?;
        io::copy(&mut src, &mut dst)?;
        dst.flush()?;
        drop(src);
        drop(dst);

        // Truncate the original log file in-place so open file descriptors held
        // by log-capture threads remain valid and continue writing to position 0.
        let log_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(log_path)?;
        drop(log_file);

        Ok(())
    }

    /// Rotate all logs for an application.
    pub fn rotate_app_logs(&self, app: &str, log_root: &Path) -> Result<()> {
        let app_log_dir = log_root.join(app);

        if !app_log_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&app_log_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only rotate .log files (not rotated ones like .log.1, .log.2)
            if path.extension().is_some_and(|ext| ext == "log") && self.needs_rotation(&path)? {
                self.rotate(&path)?;
                tracing::info!("Rotated log: {}", path.display());
            }
        }

        Ok(())
    }
}

/// Get log file size in bytes.
#[cfg(test)]
pub fn get_log_size(log_path: &Path) -> Result<u64> {
    if !log_path.exists() {
        return Ok(0);
    }
    Ok(fs::metadata(log_path)?.len())
}

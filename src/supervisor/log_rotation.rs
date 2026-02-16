//! Log rotation module for the supervisor.
//!
//! Handles automatic log file rotation based on size and retention policies.

use anyhow::Result;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Log rotation configuration.
#[derive(Debug, Clone)]
pub struct LogRotationConfig {
    /// Maximum log file size in bytes before rotation (default: 10MB)
    pub max_size: u64,
    /// Number of rotated logs to keep (default: 5)
    pub retention_count: u32,
    /// Compress rotated logs (default: false)
    #[allow(dead_code)]
    pub compress: bool,
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        LogRotationConfig {
            max_size: 10 * 1024 * 1024, // 10MB
            retention_count: 5,
            compress: false,
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
    #[allow(dead_code)]
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

        // Rotate current log
        let rotated_path = log_dir.join(format!("{}.1", log_name));

        // Copy current log to rotated (in case file is open)
        let mut src = File::open(log_path)?;
        let mut dst = File::create(&rotated_path)?;
        let mut buffer = Vec::new();
        src.read_to_end(&mut buffer)?;
        dst.write_all(&buffer)?;

        // Truncate original log file
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
            if path.extension().is_some_and(|ext| ext == "log")
                && self.needs_rotation(&path)? {
                    self.rotate(&path)?;
                    println!("Rotated log: {}", path.display());
                }
        }

        Ok(())
    }

    /// Clean up old logs beyond retention policy.
    pub fn cleanup_old_logs(&self, log_root: &Path) -> Result<()> {
        for entry in fs::read_dir(log_root)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if this is a rotated log file (e.g., app.log.5)
                    if let Some(last_dot) = file_name.rfind('.') {
                        if let Ok(num) = file_name[last_dot + 1..].parse::<u32>() {
                            if num > self.config.retention_count {
                                let _ = fs::remove_file(&path);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Get log file size in bytes.
#[allow(dead_code)]
pub fn get_log_size(log_path: &Path) -> Result<u64> {
    if !log_path.exists() {
        return Ok(0);
    }
    Ok(fs::metadata(log_path)?.len())
}

/// Get log file age in seconds.
#[allow(dead_code)]
pub fn get_log_age(log_path: &Path) -> Result<u64> {
    if !log_path.exists() {
        return Ok(0);
    }

    let metadata = fs::metadata(log_path)?;
    let modified = metadata.modified()?;
    let duration = modified.duration_since(UNIX_EPOCH)?;
    Ok(duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_log_rotator_creation() {
        let rotator = LogRotator::with_defaults();
        assert_eq!(rotator.config.max_size, 10 * 1024 * 1024);
        assert_eq!(rotator.config.retention_count, 5);
    }

    #[test]
    fn test_needs_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        // Create small log file
        let mut file = File::create(&log_path).unwrap();
        writeln!(file, "Small log entry").unwrap();

        let rotator = LogRotator::with_defaults();
        assert!(!rotator.needs_rotation(&log_path).unwrap());
    }

    #[test]
    fn test_rotate_log() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        // Create log file with content
        let mut file = File::create(&log_path).unwrap();
        writeln!(file, "Log content").unwrap();

        let rotator = LogRotator::new(LogRotationConfig {
            max_size: 0, // Force rotation
            retention_count: 3,
            compress: false,
        });

        rotator.rotate(&log_path).unwrap();

        // Original file should exist and be empty
        assert!(log_path.exists());
        assert_eq!(fs::read_to_string(&log_path).unwrap(), "");

        // Rotated file should exist with content
        let rotated_path = temp_dir.path().join("test.log.1");
        assert!(rotated_path.exists());
        assert!(fs::read_to_string(&rotated_path)
            .unwrap()
            .contains("Log content"));
    }

    #[test]
    fn test_log_size() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let mut file = File::create(&log_path).unwrap();
        writeln!(file, "Test content").unwrap();

        let size = get_log_size(&log_path).unwrap();
        assert!(size > 0);
    }
}

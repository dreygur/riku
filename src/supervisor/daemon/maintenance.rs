//! Periodic maintenance tasks: log rotation and stats writing.

use anyhow::Result;
use std::fs;

use crate::supervisor::daemon::Supervisor;

impl Supervisor {
    /// Rotate logs for all applications.
    pub(super) fn rotate_logs(&self) -> Result<()> {
        if !self.log_root.exists() {
            return Ok(());
        }

        // Rotate logs for each app
        for entry in fs::read_dir(&self.log_root)? {
            let entry = entry?;
            let app_dir = entry.path();

            if app_dir.is_dir() {
                if let Some(app_name) = app_dir.file_name().and_then(|s| s.to_str()) {
                    match self.log_rotator.rotate_app_logs(app_name, &self.log_root) {
                        Ok(_) => {
                            // Log rotation successful
                        }
                        Err(e) => {
                            tracing::error!("Failed to rotate logs for {}: {:?}", app_name, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old logs beyond retention policy.
    #[allow(dead_code)]
    pub fn cleanup_old_logs(&self) -> Result<()> {
        if !self.log_root.exists() {
            return Ok(());
        }

        self.log_rotator.cleanup_old_logs(&self.log_root)
    }

    /// Write current stats to the stats file.
    pub(super) fn write_stats(&self) -> Result<()> {
        self.process_manager
            .stats()
            .write_stats_to_file(&self.stats_file)?;
        Ok(())
    }
}

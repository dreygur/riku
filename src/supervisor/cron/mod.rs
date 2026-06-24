//! Cron scheduling module for the supervisor.
//!
//! Handles cron expression parsing and job scheduling.
//!
//! All schedules are interpreted in **UTC** — see [`parser`] for the full
//! schedule contract and time-zone rationale.

use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;

pub mod parser;

pub use parser::validate_cron_expression;

#[cfg(test)]
mod tests;

use parser::{calculate_next_run, calculate_next_run_after};

/// A scheduled cron job.
#[derive(Clone, Debug)]
pub struct CronJob {
    pub app: String,
    pub schedule: String,
    pub command: String,
    pub next_run: SystemTime,
}

impl CronJob {
    /// Create a new cron job and calculate its next run time.
    pub fn new(app: String, schedule: String, command: String) -> Result<Self> {
        let next_run = calculate_next_run(&schedule)?;
        Ok(CronJob {
            app,
            schedule,
            command,
            next_run,
        })
    }

    /// Check if this job should run now.
    pub fn should_run_now(&self) -> bool {
        SystemTime::now() >= self.next_run
    }
}

/// Cron scheduler that manages and executes scheduled jobs.
#[derive(Default)]
pub struct CronScheduler {
    jobs: HashMap<String, CronJob>, // Key: app-name-cron-index
}

impl CronScheduler {
    /// Create a new cron scheduler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cron job to the scheduler.
    pub fn add_job(
        &mut self,
        app: &str,
        index: usize,
        schedule: &str,
        command: &str,
    ) -> Result<()> {
        let job_id = format!("{}-cron-{}", app, index);
        let job = CronJob::new(app.to_string(), schedule.to_string(), command.to_string())?;
        self.jobs.insert(job_id, job);
        Ok(())
    }

    /// Remove all cron jobs belonging to an app.
    ///
    /// Called when an app's cron worker config is unloaded (stop/destroy) and
    /// before reloading its Procfile, so that removed or renumbered cron
    /// entries do not keep firing indefinitely after the app is gone.
    pub fn remove_app_jobs(&mut self, app: &str) {
        let prefix = format!("{}-cron-", app);
        self.jobs.retain(|job_id, _| !job_id.starts_with(&prefix));
    }

    /// Get all scheduled jobs.
    pub fn get_jobs(&self) -> &HashMap<String, CronJob> {
        &self.jobs
    }

    /// Mark a job as run and update its next run time.
    pub fn mark_job_run(&mut self, app: &str, index: usize) -> Result<()> {
        let job_id = format!("{}-cron-{}", app, index);
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.next_run = calculate_next_run_after(&job.schedule, job.next_run)?;
        }
        Ok(())
    }
}

//! Cron scheduling module for the supervisor.
//!
//! Handles cron expression parsing and job scheduling.

#![allow(dead_code)]

use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
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

    /// Execute the cron job command.
    pub fn execute(&self) -> Result<()> {
        tracing::info!(
            "Executing cron job for app '{}': {}",
            self.app, self.command
        );

        let output = Command::new("sh").arg("-c").arg(&self.command).output()?;

        if !output.status.success() {
            tracing::error!(
                "Cron job for app '{}' failed: {}",
                self.app,
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            tracing::info!("Cron job for app '{}' completed successfully", self.app);
        }

        Ok(())
    }

    /// Update the next run time after execution.
    pub fn update_next_run(&mut self) -> Result<()> {
        self.next_run = calculate_next_run_after(&self.schedule, self.next_run)?;
        Ok(())
    }
}

/// Cron scheduler that manages and executes scheduled jobs.
pub struct CronScheduler {
    jobs: HashMap<String, CronJob>, // Key: app-name-cron-index
}

impl CronScheduler {
    /// Create a new cron scheduler.
    pub fn new() -> Self {
        CronScheduler {
            jobs: HashMap::new(),
        }
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

    /// Remove a cron job from the scheduler.
    pub fn remove_job(&mut self, app: &str, index: usize) -> Result<()> {
        let job_id = format!("{}-cron-{}", app, index);
        self.jobs.remove(&job_id);
        Ok(())
    }

    /// Get all scheduled jobs.
    pub fn get_jobs(&self) -> &HashMap<String, CronJob> {
        &self.jobs
    }

    /// Get jobs that should run now.
    pub fn get_jobs_to_run(&self) -> Vec<&CronJob> {
        let now = SystemTime::now();
        self.jobs
            .values()
            .filter(|job| now >= job.next_run)
            .collect()
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

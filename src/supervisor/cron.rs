//! Cron scheduling module for the supervisor.
//!
//! Handles cron expression parsing and job scheduling.

#![allow(dead_code)]

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};

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
        println!(
            "Executing cron job for app '{}': {}",
            self.app, self.command
        );

        let output = Command::new("sh").arg("-c").arg(&self.command).output()?;

        if !output.status.success() {
            eprintln!(
                "Cron job for app '{}' failed: {}",
                self.app,
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            println!("Cron job for app '{}' completed successfully", self.app);
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

    /// Run the scheduler loop, checking for jobs to execute.
    pub fn run_scheduler(&mut self) -> Result<()> {
        loop {
            let now = SystemTime::now();
            let jobs_to_run: Vec<String> = self
                .jobs
                .iter()
                .filter(|(_, job)| now >= job.next_run)
                .map(|(id, _)| id.clone())
                .collect();

            for job_id in jobs_to_run {
                if let Some(job) = self.jobs.get_mut(&job_id) {
                    // Execute the job
                    if let Err(e) = job.execute() {
                        eprintln!("Error executing cron job {}: {}", job_id, e);
                    }

                    // Update next run time
                    if let Err(e) = job.update_next_run() {
                        eprintln!("Error updating next run time for job {}: {}", job_id, e);
                    }
                }
            }

            // Sleep for 1 second before checking again
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Get all scheduled jobs.
    pub fn get_jobs(&self) -> &HashMap<String, CronJob> {
        &self.jobs
    }
}

/// Parse a cron expression and calculate the next run time.
fn calculate_next_run(schedule: &str) -> Result<SystemTime> {
    let now = SystemTime::now();
    calculate_next_run_after(schedule, now)
}

/// Parse a cron expression and calculate the next run time after a given time.
fn calculate_next_run_after(schedule: &str, after: SystemTime) -> Result<SystemTime> {
    // This is a simplified implementation
    // A full implementation would properly parse cron expressions
    let parts: Vec<&str> = schedule.split_whitespace().collect();

    if parts.len() < 5 {
        return Err(anyhow::anyhow!("Invalid cron expression: {}", schedule));
    }

    // For now, we'll just return the next minute as a placeholder
    // A full implementation would calculate the actual next time based on the cron pattern
    Ok(after + Duration::from_secs(60))
}

/// Validate a cron expression.
pub fn validate_cron_expression(expr: &str) -> bool {
    let cron_regex = Regex::new(
        r#"^((\*(/\d+)?|[0-9]+(-[0-9]+)?(,[0-9]+(-[0-9]+)?)*)\s+){4}(\*(/\d+)?|[0-9]+(-[0-9]+)?(,[0-9]+(-[0-9]+)?)*)$"#,
    ).unwrap();

    cron_regex.is_match(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cron_expression() {
        assert!(validate_cron_expression("0 * * * *")); // Hourly
        assert!(validate_cron_expression("0 0 * * *")); // Daily at midnight
        assert!(validate_cron_expression("*/5 * * * *")); // Every 5 minutes
        assert!(validate_cron_expression("0 2 * * 1-5")); // 2 AM, Mon-Fri

        assert!(!validate_cron_expression("invalid"));
        assert!(!validate_cron_expression("0 * * *")); // Missing one field
    }

    #[test]
    fn test_cron_job_creation() {
        let job = CronJob::new(
            "testapp".to_string(),
            "0 * * * *".to_string(),
            "echo 'hello'".to_string(),
        )
        .unwrap();

        assert_eq!(job.app, "testapp");
        assert_eq!(job.schedule, "0 * * * *");
        assert_eq!(job.command, "echo 'hello'");
    }
}

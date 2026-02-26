//! Cron scheduling module for the supervisor.
//!
//! Handles cron expression parsing and job scheduling.

#![allow(dead_code)]

use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Timelike};

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

/// Parse a cron field (minute, hour, day, month, weekday) and return matching values.
fn parse_cron_field(field: &str, min: u32, max: u32) -> Result<Vec<u32>> {
    let mut values = Vec::new();

    for part in field.split(',') {
        if part.contains('/') {
            // Handle */n or n/m syntax
            let parts: Vec<&str> = part.split('/').collect();
            let range = parts[0];
            let step: u32 = parts[1].parse()?;

            let (start, end) = if range == "*" {
                (min, max)
            } else if range.contains('-') {
                let range_parts: Vec<&str> = range.split('-').collect();
                (range_parts[0].parse()?, range_parts[1].parse()?)
            } else {
                (range.parse()?, range.parse()?)
            };

            for v in (start..=end).step_by(step as usize) {
                if v >= min && v <= max {
                    values.push(v);
                }
            }
        } else if part.contains('-') {
            // Handle n-m range
            let parts: Vec<&str> = part.split('-').collect();
            let start: u32 = parts[0].parse()?;
            let end: u32 = parts[1].parse()?;
            for v in start..=end {
                if v >= min && v <= max {
                    values.push(v);
                }
            }
        } else if part == "*" {
            // Handle * (all values)
            for v in min..=max {
                values.push(v);
            }
        } else {
            // Single value
            let v: u32 = part.parse()?;
            if v >= min && v <= max {
                values.push(v);
            }
        }
    }

    values.sort();
    values.dedup();
    Ok(values)
}

/// Parse a cron expression and calculate the next run time.
fn calculate_next_run(schedule: &str) -> Result<SystemTime> {
    let now = SystemTime::now();
    calculate_next_run_after(schedule, now)
}

/// Parse a cron expression and calculate the next run time after a given time.
fn calculate_next_run_after(schedule: &str, after: SystemTime) -> Result<SystemTime> {
    let parts: Vec<&str> = schedule.split_whitespace().collect();

    if parts.len() < 5 {
        return Err(anyhow::anyhow!("Invalid cron expression: {}", schedule));
    }

    let minute_parts = parse_cron_field(parts[0], 0, 59)?;
    let hour_parts = parse_cron_field(parts[1], 0, 23)?;
    let day_parts = parse_cron_field(parts[2], 1, 31)?;
    let month_parts = parse_cron_field(parts[3], 1, 12)?;
    let weekday_parts = parse_cron_field(parts[4], 0, 6)?; // 0 = Sunday in cron

    // Convert after to NaiveDateTime
    let after_secs = after
        .duration_since(UNIX_EPOCH)
        .map_err(|_| anyhow::anyhow!("Time went backwards"))?
        .as_secs();
    let after_datetime = chrono::DateTime::from_timestamp(after_secs as i64, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?
        .naive_utc();

    let mut candidate = after_datetime;

    // Simple approach: iterate forward minute by minute until we find a match
    // This is not the most efficient but is correct and simple
    for _ in 0..60 * 24 * 366 {
        // Max 1 year lookahead
        candidate += Duration::from_secs(60);

        let minute = candidate.minute();
        let hour = candidate.hour();
        let day = candidate.day();
        let month = candidate.month();
        let weekday = candidate.weekday().num_days_from_sunday();

        if minute_parts.contains(&minute)
            && hour_parts.contains(&hour)
            && (day_parts.contains(&day) || weekday_parts.contains(&weekday))
            && month_parts.contains(&month)
        {
            // Convert NaiveDateTime back to SystemTime
            let timestamp = candidate.and_utc().timestamp();
            return Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp as u64));
        }
    }

    // Fallback: return 1 hour from now
    Ok(after + Duration::from_secs(3600))
}

/// Validate a cron expression.
pub fn validate_cron_expression(expr: &str) -> bool {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() < 5 {
        return false;
    }

    // Try to parse each field - if any fails, it's invalid
    parse_cron_field(parts[0], 0, 59).is_ok()
        && parse_cron_field(parts[1], 0, 23).is_ok()
        && parse_cron_field(parts[2], 1, 31).is_ok()
        && parse_cron_field(parts[3], 1, 12).is_ok()
        && parse_cron_field(parts[4], 0, 6).is_ok()
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

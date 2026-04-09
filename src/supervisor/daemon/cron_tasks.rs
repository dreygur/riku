//! Cron job loading and execution tasks for the supervisor daemon.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::supervisor::config::WorkerConfig;
use crate::supervisor::daemon::Supervisor;

impl Supervisor {
    /// Check and execute cron jobs that are due.
    ///
    /// Each due job is run in its own thread so it cannot block the supervisor
    /// main loop. The `next_run` time is updated using the exact `job_id` so
    /// that all jobs for the same app are advanced independently (Bug #5 fix).
    /// Working directory and environment variables are taken from the app's
    /// worker config in `workers-enabled/` when available.
    pub(super) fn check_cron_jobs(&mut self) -> Result<()> {
        // Collect (job_id, app, command) for every job that is due.
        // We clone here to avoid holding an immutable borrow while we later
        // call mark_job_run (which takes &mut self).
        let jobs_to_run: Vec<(String, String, String)> = self
            .cron_scheduler
            .get_jobs()
            .iter()
            .filter(|(_id, job)| job.should_run_now())
            .map(|(id, job)| (id.clone(), job.app.clone(), job.command.clone()))
            .collect();

        for (job_id, app, command) in jobs_to_run {
            // Try to find the app's working directory and env vars from any
            // existing worker config file so the cron command has the right
            // context (e.g. virtualenv PATH, DATABASE_URL, etc.).
            let (working_dir, env_vars) = self.get_app_context(&app);

            // Get resource limits from ProcessManager to apply to cron jobs
            let limits = self.process_manager.get_resource_limits().clone();

            // Execute cron job in thread pool to prevent unbounded thread creation.
            // Thread pool limits concurrent cron jobs to prevent resource exhaustion.
            let app_clone = app.clone();
            let job_id_clone = job_id.clone();
            self.cron_thread_pool.execute(move || {
                use std::os::unix::process::CommandExt;

                let mut cmd = std::process::Command::new("sh");
                cmd.arg("-c").arg(&command);

                // Apply resource limits to cron jobs for security
                unsafe {
                    cmd.pre_exec(move || {
                        limits.apply()?;
                        Ok(())
                    });
                }

                if let Some(ref dir) = working_dir {
                    cmd.current_dir(dir);
                }
                for (k, v) in &env_vars {
                    cmd.env(k, v);
                }
                match cmd.output() {
                    Ok(out) if out.status.success() => {
                        tracing::info!(
                            "Cron job {} for app '{}' completed successfully",
                            job_id_clone,
                            app_clone
                        );
                    }
                    Ok(out) => {
                        tracing::error!(
                            "Cron job {} for app '{}' failed: {}",
                            job_id_clone,
                            app_clone,
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Error executing cron job {} for app '{}': {}",
                            job_id_clone,
                            app_clone,
                            e
                        );
                    }
                }
            });

            // Update next_run using the exact job_id (Bug #5 fix: each job
            // is advanced independently, not just the first matching one).
            if let Some(idx) = job_id.rsplit('-').next() {
                if let Ok(index) = idx.parse::<usize>() {
                    if let Err(e) = self.cron_scheduler.mark_job_run(&app, index) {
                        tracing::error!("Failed to update next_run for cron job {}: {}", job_id, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Return the working directory and environment variables for an app by
    /// reading the first matching worker config file from workers-enabled/.
    pub(super) fn get_app_context(&self, app: &str) -> (Option<String>, HashMap<String, String>) {
        let workers_enabled = self.config_dir.clone();
        let mut env_vars: HashMap<String, String> = HashMap::new();
        let mut working_dir: Option<String> = None;

        if let Ok(entries) = fs::read_dir(&workers_enabled) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                    continue;
                }
                let fname = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if !fname.starts_with(&format!("{}-", app)) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(cfg) = toml::from_str::<WorkerConfig>(&content) {
                        working_dir = Some(cfg.options.working_dir.clone());
                        env_vars = cfg.env.clone();
                        break;
                    }
                }
            }
        }

        (working_dir, env_vars)
    }

    /// Load cron jobs from an app's Procfile.
    pub fn load_cron_jobs(&mut self, app: &str, procfile_path: &Path) -> Result<()> {
        if !procfile_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(procfile_path)?;
        let mut cron_index = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(pos) = line.find(':') {
                let kind = line[..pos].trim();
                let command = line[pos + 1..].trim();

                if kind.starts_with("cron") {
                    // The kind is like "cron0", "cron1", etc.
                    // We don't need the number, just that it starts with "cron"

                    // Parse the command as a cron expression followed by the command
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    if parts.len() >= 5 {
                        // This is a valid cron expression + command
                        let schedule = parts[..5].join(" ");
                        let actual_command = parts[5..].join(" ");

                        if crate::supervisor::cron::validate_cron_expression(&schedule) {
                            self.cron_scheduler.add_job(
                                app,
                                cron_index,
                                &schedule,
                                &actual_command,
                            )?;
                            tracing::info!(
                                "Loaded cron job for app '{}': {} {}",
                                app,
                                schedule,
                                actual_command
                            );
                            cron_index += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

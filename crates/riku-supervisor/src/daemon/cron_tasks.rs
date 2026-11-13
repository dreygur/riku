//! Cron job loading and execution tasks for the supervisor daemon.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::{app_worker_configs, WorkerConfig};
use crate::daemon::Supervisor;

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

            let limits = self
                .process_manager
                .get_resource_limits()
                .with_app_env(&env_vars);

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

    /// Return the working directory and environment variables for an app's
    /// cron jobs, from its cron worker config in workers-enabled/.
    ///
    /// A web copy's config also holds its PORT and socket variables, so it is
    /// used only when the app has no readable cron config, and then the
    /// first readable config in name order is.
    pub(super) fn get_app_context(&self, app: &str) -> (Option<String>, HashMap<String, String>) {
        let configs: Vec<WorkerConfig> = app_worker_configs(&self.config_dir, app)
            .iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .filter_map(|content| toml::from_str::<WorkerConfig>(&content).ok())
            .collect();
        let cron = configs
            .iter()
            .position(|cfg| cfg.worker.kind.starts_with("cron"));
        match configs.into_iter().nth(cron.unwrap_or(0)) {
            Some(cfg) => (Some(cfg.options.working_dir), cfg.env),
            None => (None, HashMap::new()),
        }
    }

    /// Schedule an app's cron jobs, given as `(kind, command)` in a stable
    /// order (`app_config::load_cron_entries`: riku.toml `[cron]` sorted, or
    /// every Procfile `cron*` line in file order, repeated kinds included).
    ///
    /// Earlier jobs of the app are dropped first, so an edit that removes or
    /// renumbers cron entries leaves no stale job firing on the old schedule.
    #[allow(clippy::question_mark)]
    pub fn load_cron_jobs(&mut self, app: &str, entries: &[(String, String)]) -> Result<()> {
        self.cron_scheduler.remove_app_jobs(app);

        let mut cron_index = 0;
        for (_, line) in entries {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            let schedule = parts[..5].join(" ");
            let command = parts[5..].join(" ");
            if !crate::cron::validate_cron_expression(&schedule) {
                continue;
            }
            if let Err(e) = self
                .cron_scheduler
                .add_job(app, cron_index, &schedule, &command)
            {
                return Err(e);
            }
            tracing::info!(
                "Loaded cron job for app '{}': {} {}",
                app,
                schedule,
                command
            );
            cron_index += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "cron_tasks_tests.rs"]
mod tests;

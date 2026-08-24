//! Configuration watcher: loads, reloads, and unloads TOML worker configs for the supervisor.

use anyhow::Result;
use notify::Event;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::PoisonError;

use crate::config::WorkerConfig;
use crate::daemon::Supervisor;
use crate::CONFIG_RELOAD_LOCK;

impl Supervisor {
    /// Reload all configurations - stop removed configs, start new/modified ones.
    pub(super) fn reload_all_configs(&mut self) -> Result<()> {
        // Acquire lock to prevent race with file watcher events. A prior
        // panic while holding this lock must not permanently break every
        // future reload, so recover the poisoned guard instead of unwrapping.
        let _lock = CONFIG_RELOAD_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        // Scan directory for current config files
        let mut current_configs: HashMap<String, std::path::PathBuf> = HashMap::new();

        if self.config_dir.exists() {
            for entry in fs::read_dir(&self.config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        current_configs.insert(filename.to_string(), path);
                    }
                }
            }
        }

        // Stop processes for configs that no longer exist
        let configs_to_remove: Vec<String> = self
            .watched_configs
            .keys()
            .filter(|k| !current_configs.contains_key(*k))
            .cloned()
            .collect();

        for filename in &configs_to_remove {
            tracing::info!("Config file removed: {}", filename);
            self.unload_config(filename)?;
            self.watched_configs.remove(filename);
        }

        // Load new or modified configs
        for (filename, path) in current_configs {
            if let Some(_old_modified) = self.watched_configs.get(&filename) {
                // Config already loaded, check if modified
                if let Ok(new_metadata) = fs::metadata(&path) {
                    if let Ok(new_modified) = new_metadata.modified() {
                        // Compare with stored modification time
                        if new_modified > *_old_modified {
                            tracing::info!("Config file modified: {}", filename);
                            if let Err(e) = self.handle_modified_config(&path, &filename) {
                                tracing::error!("Error reloading config {}: {}", filename, e);
                            }
                            self.watched_configs.insert(filename, new_modified);
                        }
                    }
                }
            } else {
                // New config
                tracing::info!("New config file detected: {}", filename);
                if let Err(e) = self.load_config_file(&path, &filename) {
                    tracing::error!("Error loading config {}: {}", filename, e);
                }
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        self.watched_configs.insert(filename, modified);
                    }
                }
            }
        }

        let new_count = self.process_manager.get_process_count();
        tracing::info!("Reload complete. Managing {} processes", new_count);
        Ok(())
    }

    /// Load all existing configurations from the config directory.
    pub(super) fn load_initial_configs(&mut self) -> Result<()> {
        if !self.config_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    self.load_config_file(&path, filename)?;
                    self.watched_configs
                        .insert(filename.to_string(), fs::metadata(&path)?.modified()?);
                }
            }
        }

        Ok(())
    }

    /// Handle file system events (create, modify, remove config files).
    pub(super) fn handle_file_event(&mut self, event: Event) -> Result<()> {
        // Acquire lock to prevent race with manual reload (SIGHUP). See
        // `reload_all_configs` for why a poisoned lock is recovered here.
        let _lock = CONFIG_RELOAD_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        for path in event.paths {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    match event.kind {
                        notify::EventKind::Create(_) => {
                            tracing::info!("New config file detected: {}", filename);
                            self.load_config_file(&path, filename)?;
                            if let Ok(metadata) = fs::metadata(&path) {
                                if let Ok(modified) = metadata.modified() {
                                    self.watched_configs.insert(filename.to_string(), modified);
                                }
                            }
                        }
                        notify::EventKind::Modify(_) => {
                            if let Ok(metadata) = fs::metadata(&path) {
                                if let Ok(new_modified) = metadata.modified() {
                                    if let Some(old_modified) = self.watched_configs.get(filename) {
                                        if new_modified > *old_modified {
                                            tracing::info!("Config file modified: {}", filename);
                                            self.handle_modified_config(&path, filename)?;
                                            self.watched_configs
                                                .insert(filename.to_string(), new_modified);
                                        }
                                    } else {
                                        // File not yet tracked (e.g. atomic-write editors send
                                        // Modify before Create). Treat as a new config.
                                        tracing::info!(
                                            "New config file detected via Modify: {}",
                                            filename
                                        );
                                        self.load_config_file(&path, filename)?;
                                        self.watched_configs
                                            .insert(filename.to_string(), new_modified);
                                    }
                                }
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            tracing::info!("Config file removed: {}", filename);
                            self.unload_config(filename)?;
                            self.watched_configs.remove(filename);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse a worker config TOML file without spawning anything.
    fn parse_worker_config(path: &Path) -> Result<WorkerConfig> {
        let config_content = fs::read_to_string(path).map_err(|e| {
            tracing::error!("Error reading config file {}: {}", path.display(), e);
            e
        })?;
        toml::from_str(&config_content).map_err(|e| {
            tracing::error!("Error parsing config file {}: {}", path.display(), e);
            e.into()
        })
    }

    /// Handle a modified worker config.
    ///
    /// If the process is already running and the new config has a health
    /// check configured, deploy it as a probed canary generation instead of
    /// tearing the running process down: `deploy_generation` spawns the new
    /// version alongside the old one and only swaps it in once it passes
    /// its probe window, with the rollback circuit breaker handling
    /// failures. Otherwise (no health check, or not currently running)
    /// fall back to the original unload-then-respawn behavior.
    pub(super) fn handle_modified_config(&mut self, path: &Path, filename: &str) -> Result<()> {
        let worker_config = Self::parse_worker_config(path)?;

        if worker_config.worker.kind.starts_with("cron") {
            return self.load_config_file(path, filename);
        }

        let process_id = format!(
            "{}-{}-{}",
            worker_config.worker.app, worker_config.worker.kind, worker_config.worker.ordinal
        );

        if worker_config.options.health_check.is_some()
            && self.process_manager.is_managed(&process_id)
        {
            return self
                .process_manager
                .deploy_generation(&process_id, worker_config);
        }

        self.unload_config(filename)?;
        self.load_config_file(path, filename)
    }

    /// Load and start a configuration from a TOML file.
    pub(super) fn load_config_file(&mut self, path: &Path, _filename: &str) -> Result<()> {
        let worker_config = Self::parse_worker_config(path)?;

        // If this is a cron worker, load cron jobs from the app's Procfile instead of
        // spawning a persistent process (cron entries are driven by the scheduler).
        if worker_config.worker.kind.starts_with("cron") {
            let procfile_path =
                std::path::Path::new(&worker_config.options.working_dir).join("Procfile");
            let app = &worker_config.worker.app.clone();
            if let Err(e) = self.load_cron_jobs(app, &procfile_path) {
                tracing::error!("Error loading cron jobs for {}: {}", app, e);
            }
            return Ok(());
        }

        if let Err(e) = self.process_manager.spawn_process(&worker_config) {
            tracing::error!(
                "Error spawning process for {}: {}",
                worker_config.worker.app,
                e
            );
            return Err(e);
        }
        Ok(())
    }

    /// Stop and remove a configuration.
    ///
    /// Stops exactly the one process this config file described, NOT every
    /// process belonging to the app. A single worker config can be removed
    /// on its own (scaling one ordinal down, or a per-instance delete from
    /// the dashboard), and this used to tear down every sibling ordinal too
    /// (via `stop_app_processes`), turning e.g. "scale web from 5 to 3" into
    /// a brief full outage until the following recreate step respawned
    /// 1..3 again. `riku stop`/`riku destroy` still stop everything: they
    /// remove *all* of an app's config files, so this runs once per file and
    /// the net effect is unchanged for that case.
    // Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
    #[allow(clippy::question_mark)]
    pub(super) fn unload_config(&mut self, filename: &str) -> Result<()> {
        // Worker config filenames are <app>-<kind>-<ordinal>.toml, which is
        // exactly the process_id format `stop_process_by_id` expects.
        let process_id = filename.strip_suffix(".toml").unwrap_or(filename);
        // Strip "-<ordinal>" (last component)
        let without_ordinal = process_id
            .rsplit_once('-')
            .map(|x| x.0)
            .unwrap_or(process_id);
        // Strip "-<kind>" (now last component): the app name itself may
        // contain hyphens (e.g. "my-app"), hence stripping from the right.
        let (app_name, kind) = without_ordinal
            .rsplit_once('-')
            .unwrap_or((without_ordinal, ""));

        if kind.starts_with("cron") {
            // Cron jobs live in the in-memory scheduler, not the process
            // manager. There's no scaling concept for cron workers (always
            // ordinal 1), so removing a cron config always means "this
            // app's cron entries changed or it's stopping": purging all of
            // the app's jobs here is correct, not over-eager like the
            // process case would be.
            self.cron_scheduler.remove_app_jobs(app_name);
        } else {
            self.process_manager.stop_process_by_id(process_id)?;
        }

        // `riku stop` removes worker configs but leaves the app's source
        // directory in place (so a later deploy/restart can recreate
        // them): its stats should persist as `[STOPPED]` until then.
        // `riku destroy` removes the app directory too. Only in the
        // latter case should the stats entries be purged; otherwise every
        // destroyed app leaves a permanent ghost row in `/metrics` that
        // nothing ever clears.
        let paths = match riku_config::RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => return Err(e),
        };
        if !paths.app_root.join(app_name).exists() {
            self.process_manager.stats_mut().remove_app(app_name);
            self.cron_scheduler.remove_app_jobs(app_name);
        }
        Ok(())
    }
}

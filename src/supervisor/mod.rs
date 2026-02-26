//! Supervisor daemon module for managing application processes.
//!
//! This module implements a process supervisor that replaces uWSGI Emperor,
//! handling process lifecycle, monitoring, and restart logic.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

use notify::{Event, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub mod config;
pub mod cron;
pub mod log_rotation;
pub mod process;
pub mod stats;

use config::WorkerConfig;
use cron::CronScheduler;
use log_rotation::{LogRotationConfig, LogRotator};
use process::ProcessManager;

/// Main supervisor daemon that monitors worker configurations and manages processes.
pub struct Supervisor {
    config_dir: std::path::PathBuf,
    process_manager: ProcessManager,
    watched_configs: HashMap<String, std::time::SystemTime>,
    log_rotator: LogRotator,
    log_root: std::path::PathBuf,
    last_log_rotation: std::time::SystemTime,
    log_rotation_interval: Duration,
    stats_file: std::path::PathBuf,
    last_stats_write: std::time::SystemTime,
    stats_write_interval: Duration,
    cron_scheduler: CronScheduler,
    last_cron_check: std::time::SystemTime,
    cron_check_interval: Duration,
}

impl Supervisor {
    /// Create a new supervisor instance.
    pub fn new(config_dir: std::path::PathBuf) -> Result<Self> {
        // Determine log root from config_dir (go up one level to .riku, then into logs)
        let log_root = config_dir
            .parent()
            .map(|p| p.join("logs"))
            .unwrap_or_else(|| std::path::PathBuf::from("./logs"));

        // Determine stats file path (in .riku root)
        let stats_file = config_dir
            .parent()
            .map(|p| p.join("stats.json"))
            .unwrap_or_else(|| std::path::PathBuf::from("./stats.json"));

        Ok(Supervisor {
            config_dir,
            process_manager: ProcessManager::new()?,
            watched_configs: HashMap::new(),
            log_rotator: LogRotator::new(LogRotationConfig::default()),
            log_root,
            last_log_rotation: std::time::SystemTime::now(),
            log_rotation_interval: Duration::from_secs(300), // Check every 5 minutes
            stats_file,
            last_stats_write: std::time::SystemTime::now(),
            stats_write_interval: Duration::from_secs(5), // Write stats every 5 seconds
            cron_scheduler: CronScheduler::new(),
            last_cron_check: std::time::SystemTime::now(),
            cron_check_interval: Duration::from_secs(10), // Check cron jobs every 10 seconds
        })
    }

    /// Start the supervisor daemon loop.
    pub fn run(&mut self) -> Result<()> {
        println!("Starting riku supervisor daemon...");
        println!("Monitoring: {}", self.config_dir.display());
        println!("Press Ctrl+C to stop");

        // Set up signal handlers for graceful shutdown
        setup_signal_handlers()?;

        // Load existing configurations at startup
        self.load_initial_configs()?;

        let initial_count = self.process_manager.get_process_count();
        println!("Loaded {} worker configurations", initial_count);

        // Set up file watcher for config directory with symlink following enabled
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::RecommendedWatcher::new(
            tx,
            notify::Config::default().with_follow_symlinks(true),
        )?;
        watcher.watch(&self.config_dir, RecursiveMode::NonRecursive)?;

        println!("Supervisor running. Waiting for configuration changes...");

        // Main event loop
        loop {
            // Check if we should shut down
            if !is_running() {
                println!("\nShutting down supervisor...");
                break;
            }

            // Check if reload was requested via SIGHUP
            if RELOAD_REQUESTED.load(Ordering::SeqCst) {
                println!("Reloading all configurations...");
                self.reload_all_configs()?;
                RELOAD_REQUESTED.store(false, Ordering::SeqCst);
            }

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    self.handle_file_event(event?)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic maintenance tasks - check process health
                    self.process_manager.check_processes()?;

                    // Check if it's time for log rotation
                    if self
                        .last_log_rotation
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.log_rotation_interval
                    {
                        if let Err(e) = self.rotate_logs() {
                            eprintln!("Log rotation error: {:?}", e);
                        }
                        self.last_log_rotation = std::time::SystemTime::now();
                    }

                    // Check if it's time to write stats
                    if self
                        .last_stats_write
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.stats_write_interval
                    {
                        if let Err(e) = self.write_stats() {
                            eprintln!("Failed to write stats: {:?}", e);
                        }
                        self.last_stats_write = std::time::SystemTime::now();
                    }

                    // Check if it's time to check cron jobs
                    if self
                        .last_cron_check
                        .elapsed()
                        .unwrap_or(Duration::from_secs(0))
                        >= self.cron_check_interval
                    {
                        if let Err(e) = self.check_cron_jobs() {
                            eprintln!("Cron job check error: {:?}", e);
                        }
                        self.last_cron_check = std::time::SystemTime::now();
                    }
                }
                Err(e) => {
                    eprintln!("Watcher error: {:?}", e);
                }
            }
        }

        // Clean shutdown - stop all managed processes
        println!("Stopping all managed processes...");
        self.process_manager.stop_all_processes()?;

        Ok(())
    }

    /// Reload all configurations - stop removed configs, start new/modified ones.
    fn reload_all_configs(&mut self) -> Result<()> {
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
            println!("Config file removed: {}", filename);
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
                            println!("Config file modified: {}", filename);
                            self.unload_config(&filename)?;
                            self.load_config_file(&path, &filename)?;
                            self.watched_configs.insert(filename, new_modified);
                        }
                    }
                }
            } else {
                // New config
                println!("New config file detected: {}", filename);
                self.load_config_file(&path, &filename)?;
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        self.watched_configs.insert(filename, modified);
                    }
                }
            }
        }

        let new_count = self.process_manager.get_process_count();
        println!("Reload complete. Managing {} processes", new_count);
        Ok(())
    }

    /// Load all existing configurations from the config directory.
    fn load_initial_configs(&mut self) -> Result<()> {
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
    fn handle_file_event(&mut self, event: Event) -> Result<()> {
        for path in event.paths {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    match event.kind {
                        notify::EventKind::Create(_) => {
                            println!("New config file detected: {}", filename);
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
                                            println!("Config file modified: {}", filename);
                                            // Reload the config
                                            self.unload_config(filename)?;
                                            self.load_config_file(&path, filename)?;
                                            self.watched_configs
                                                .insert(filename.to_string(), new_modified);
                                        }
                                    }
                                }
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            println!("Config file removed: {}", filename);
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

    /// Load and start a configuration from a TOML file.
    fn load_config_file(&mut self, path: &Path, _filename: &str) -> Result<()> {
        let config_content = fs::read_to_string(path)?;
        let worker_config: WorkerConfig = toml::from_str(&config_content)?;

        // If this is a cron worker, load cron jobs from the app's Procfile instead of
        // spawning a persistent process (cron entries are driven by the scheduler).
        if worker_config.worker.kind.starts_with("cron") {
            let procfile_path =
                std::path::Path::new(&worker_config.options.working_dir).join("Procfile");
            let app = &worker_config.worker.app.clone();
            self.load_cron_jobs(app, &procfile_path)?;
            return Ok(());
        }

        self.process_manager.spawn_process(&worker_config)?;
        Ok(())
    }

    /// Stop and remove a configuration.
    fn unload_config(&mut self, filename: &str) -> Result<()> {
        // Extract app name from filename (remove .toml extension)
        let app_name = filename.strip_suffix(".toml").unwrap_or(filename);
        self.process_manager.stop_app_processes(app_name)?;
        Ok(())
    }

    /// Rotate logs for all applications.
    fn rotate_logs(&self) -> Result<()> {
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
                            eprintln!("Failed to rotate logs for {}: {:?}", app_name, e);
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
    fn write_stats(&self) -> Result<()> {
        self.process_manager
            .stats()
            .write_stats_to_file(&self.stats_file)?;
        Ok(())
    }

    /// Check and execute cron jobs that are due.
    fn check_cron_jobs(&mut self) -> Result<()> {
        // Collect jobs to run first (we need to clone to avoid borrow issues)
        let jobs_to_run: Vec<(String, String, String)> = self
            .cron_scheduler
            .get_jobs_to_run()
            .iter()
            .map(|j| (j.app.clone(), j.schedule.clone(), j.command.clone()))
            .collect();

        // Execute jobs
        for (app, _schedule, command) in jobs_to_run {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    println!("Cron job for app '{}' completed successfully", app);
                }
                Ok(out) => {
                    eprintln!(
                        "Cron job for app '{}' failed: {}",
                        app,
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
                Err(e) => {
                    eprintln!("Error executing cron job for app '{}': {}", app, e);
                }
            }

            // Update next run time - get job index from the scheduler
            if let Some(job_id) = self
                .cron_scheduler
                .get_jobs()
                .keys()
                .find(|k| k.starts_with(&format!("{}-cron-", app)))
            {
                if let Some(idx) = job_id.rsplit('-').next() {
                    if let Ok(index) = idx.parse::<usize>() {
                        let _ = self.cron_scheduler.mark_job_run(&app, index);
                    }
                }
            }
        }

        // Remove apps that no longer have cron workers from scheduler
        // (This would require access to the current Procfile, which we don't have here)
        // For now, cron jobs persist until supervisor restart

        Ok(())
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

                        if cron::validate_cron_expression(&schedule) {
                            self.cron_scheduler.add_job(
                                app,
                                cron_index,
                                &schedule,
                                &actual_command,
                            )?;
                            println!(
                                "Loaded cron job for app '{}': {} {}",
                                app, schedule, actual_command
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

/// Signal handler for graceful shutdown
pub fn setup_signal_handlers() -> Result<()> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, Signal};

        extern "C" fn handle_sigterm(_: i32) {
            println!("Received SIGTERM, shutting down gracefully...");
            RUNNING.store(false, Ordering::SeqCst);
        }

        extern "C" fn handle_sigint(_: i32) {
            println!("Received SIGINT, shutting down gracefully...");
            RUNNING.store(false, Ordering::SeqCst);
        }

        extern "C" fn handle_sighup(_: i32) {
            println!("Received SIGHUP, reloading configurations...");
            RELOAD_REQUESTED.store(true, Ordering::SeqCst);
        }

        unsafe {
            sigaction(
                Signal::SIGTERM,
                &SigAction::new(
                    SigHandler::Handler(handle_sigterm),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
            sigaction(
                Signal::SIGINT,
                &SigAction::new(
                    SigHandler::Handler(handle_sigint),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
            sigaction(
                Signal::SIGHUP,
                &SigAction::new(
                    SigHandler::Handler(handle_sighup),
                    SaFlags::empty(),
                    nix::sys::signal::SigSet::empty(),
                ),
            )?;
        }
    }

    Ok(())
}

/// Check if the supervisor should continue running
pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

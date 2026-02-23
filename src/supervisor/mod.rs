//! Supervisor daemon module for managing application processes.
//!
//! This module implements a process supervisor that replaces uWSGI Emperor,
//! handling process lifecycle, monitoring, and restart logic.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

use notify::{recommended_watcher, Event, RecursiveMode, Watcher};
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
}

impl Supervisor {
    /// Create a new supervisor instance.
    pub fn new(config_dir: std::path::PathBuf) -> Result<Self> {
        // Determine log root from config_dir (go up one level to .riku, then into logs)
        let log_root = config_dir
            .parent()
            .map(|p| p.join("logs"))
            .unwrap_or_else(|| std::path::PathBuf::from("./logs"));

        Ok(Supervisor {
            config_dir,
            process_manager: ProcessManager::new()?,
            watched_configs: HashMap::new(),
            log_rotator: LogRotator::new(LogRotationConfig::default()),
            log_root,
            last_log_rotation: std::time::SystemTime::now(),
            log_rotation_interval: Duration::from_secs(300), // Check every 5 minutes
        })
    }

    /// Start the supervisor daemon loop.
    pub fn run(&mut self) -> Result<()> {
        println!("Starting riku supervisor daemon...");
        println!("Monitoring: {}", self.config_dir.display());
        println!("Press Ctrl+C to stop");

        // Set up signal handlers for graceful shutdown
        setup_signal_handlers()?;

        // Load existing configurations
        self.load_initial_configs()?;

        let initial_count = self.process_manager.get_process_count();
        println!("Loaded {} worker configurations", initial_count);

        // Set up file watcher for config directory
        let (tx, rx) = mpsc::channel();
        let mut watcher = recommended_watcher(tx)?;
        watcher.watch(&self.config_dir, RecursiveMode::NonRecursive)?;

        println!("Supervisor running. Waiting for configuration changes...");

        // Main event loop
        loop {
            // Check if we should shut down
            if !is_running() {
                println!("\nShutting down supervisor...");
                break;
            }

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    self.handle_file_event(event?)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic maintenance tasks
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
            // In a real implementation, this would trigger a config reload
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

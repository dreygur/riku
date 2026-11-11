//! Supervisor startup diagnostics, PID locking, health endpoint, and watcher setup.

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::sync::mpsc;

use super::Supervisor;

/// Whether the supervisor should treat startup diagnostics as production
/// incidents (escalate to `error` + stderr) rather than dev-environment
/// noise (`warn` only).
///
/// Defaults to production: riku's only real deployment target is a
/// long-running PaaS host, so the safer default is to surface
/// infrastructure problems loudly. Set `RIKU_ENV=development` (or `dev`)
/// when running the supervisor locally against a sandbox without cgroup v2
/// delegated, where this check is expected to fail.
fn is_production_mode() -> bool {
    !matches!(
        std::env::var("RIKU_ENV").as_deref(),
        Ok("development") | Ok("dev")
    )
}

/// Production escalates to error level and also prints to stderr, since
/// operators often tail redirected output rather than the log filter.
fn report_startup_problem(text: &str) {
    if is_production_mode() {
        tracing::error!("{text}");
        eprintln!("{text}");
    } else {
        tracing::warn!("{text}");
    }
}

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
impl Supervisor {
    /// Create the PID file with an exclusive lock, so a second supervisor
    /// can't start against the same config directory.
    pub(super) fn acquire_pid_lock(&mut self) -> Result<()> {
        let my_pid = std::process::id();
        match self.create_pid_file_with_lock(my_pid) {
            Ok(file) => {
                self.pid_file_lock = Some(file);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Another supervisor is already running (PID file locked): {}",
                e
            )),
        }
    }

    /// Take over the supervisor's delegated cgroup so isolated workers can
    /// get their own. Non-fatal: isolation is opt-in per worker, so a riku
    /// deployment that never uses it still runs, and the diagnostic names
    /// the fix at boot instead of at the first isolated deploy.
    pub(super) fn check_cgroup_isolation(&mut self) {
        match crate::cgroups::take_over_own() {
            Ok((tree, missing)) => {
                if !missing.is_empty() {
                    report_startup_problem(&format!(
                        "cgroup controllers not delegated to riku: {}; isolated workers \
                         that set those limits will fail to start",
                        missing.join(", ")
                    ));
                }
                self.process_manager.set_cgroup_tree(tree);
            }
            Err(e) => report_startup_problem(&crate::cgroups::startup_diagnostic(&e)),
        }
    }

    /// Recreate the uWSGI socket directory if it is gone, as after a reboot
    /// on a host without systemd-tmpfiles. Non-fatal: the deploy user can
    /// rarely write `/run`, and apps without uWSGI copies never need it, so
    /// a failure is reported and the supervisor carries on.
    pub(super) fn ensure_socket_root(&self) {
        let paths = match riku_config::RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => {
                tracing::warn!("cannot resolve riku paths to check the socket directory: {e}");
                return;
            }
        };
        if let Err(e) =
            riku_util::socket_dir::ensure_exists(&paths.socket_root, riku_config::SOCKET_GROUP)
        {
            tracing::error!("{e}");
        }
    }

    /// Start the health-check HTTP server, storing the metrics broadcast
    /// sender on success so the event loop can push SSE frames to it later.
    /// Non-fatal on failure: the supervisor still runs without a health
    /// endpoint, just without live metrics/control-plane access.
    pub(super) fn start_health_endpoint(&mut self) {
        let health_port = std::env::var("RIKU_HEALTH_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9091);

        if let Ok(tx) = crate::health::start_health_server(
            health_port,
            self.health_running.clone(),
            self.start_time,
            self.stats_file.clone(),
            self.control_token_file.clone(),
            self.actions.clone(),
        ) {
            self.metrics_broadcast_tx = Some(tx);
        } else {
            tracing::warn!("Failed to start health server on port {}", health_port);
        }
    }

    /// Set up the filesystem watcher on `config_dir`. The returned watcher
    /// must be kept alive by the caller for as long as `rx` is read from,
    /// dropping it stops the watch.
    #[allow(clippy::type_complexity)]
    pub(super) fn watch_config_dir(
        &self,
    ) -> Result<(
        notify::RecommendedWatcher,
        mpsc::Receiver<notify::Result<notify::Event>>,
    )> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::RecommendedWatcher::new(
            tx,
            notify::Config::default().with_follow_symlinks(true),
        ) {
            Ok(w) => w,
            Err(e) => return Err(e.into()),
        };
        if let Err(e) = watcher.watch(&self.config_dir, RecursiveMode::NonRecursive) {
            return Err(e.into());
        }
        Ok((watcher, rx))
    }
}

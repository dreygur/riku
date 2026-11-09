//! Periodic check for compose services whose image tracks a registry tag
//! (e.g. `ghcr.io/org/app:latest`), so a freshly pushed image is picked up
//! without a git push or any inbound connection to this host.
//!
//! Each check calls the `pull_service` action, which pulls the image and
//! replaces the service's copies only when the pulled image id differs from
//! what the running copies run.

use anyhow::Result;

use crate::daemon::Supervisor;
use riku_config::RikuPaths;

impl Supervisor {
    /// Submit a `pull_service` check for every app declaring
    /// `RIKU_WATCH_SERVICES`, one per configured service. Runs on the shared
    /// cron thread pool so a slow or hanging registry pull can't stall the
    /// main loop's health-check and log-rotation timing.
    // Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
    #[allow(clippy::question_mark)]
    pub(super) fn check_watched_images(&self) -> Result<()> {
        let paths = match RikuPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => return Err(e),
        };
        if !paths.env_root.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&paths.env_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(app) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };

            for service in watched_services(&paths, &app) {
                let actions = self.actions.clone();
                let paths = paths.clone();
                let app = app.clone();
                self.cron_thread_pool.execute(move || {
                    if let Err(e) = actions.pull_service(&paths, &app, &service) {
                        tracing::warn!(
                            "Watched image check failed for '{}' service '{}': {}",
                            app,
                            service,
                            e
                        );
                    }
                });
            }
        }

        Ok(())
    }
}

/// `RIKU_WATCH_SERVICES` (comma separated compose service names) from the
/// app's merged config. Empty if unset. An app whose config cannot be read
/// is skipped for this tick with a warning naming it.
fn watched_services(paths: &RikuPaths, app: &str) -> Vec<String> {
    let env = match riku_config::app_config::load_env(paths, app) {
        Ok(loaded) => loaded.env,
        Err(e) => {
            tracing::warn!("skipping image watch for '{}': {:#}", app, e);
            return Vec::new();
        }
    };
    match env.get("RIKU_WATCH_SERVICES") {
        Some(value) => value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_services_parses_comma_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        std::fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        std::fs::write(
            paths.env_root.join("myapp").join("ENV"),
            "RIKU_WATCH_SERVICES=web, worker\nOTHER=1\n",
        )
        .unwrap();

        assert_eq!(
            watched_services(&paths, "myapp"),
            vec!["web".to_string(), "worker".to_string()]
        );
    }

    #[test]
    fn watched_services_empty_when_unset() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        std::fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        std::fs::write(paths.env_root.join("myapp").join("ENV"), "OTHER=1\n").unwrap();

        assert!(watched_services(&paths, "myapp").is_empty());
    }
}

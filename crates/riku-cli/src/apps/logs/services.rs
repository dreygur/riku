//! The services `riku logs` knows for an app: its declared process kinds
//! (riku.toml or Procfile), its compose services and whatever has a log file
//! already. A service that runs but has printed nothing yet has no log file,
//! so the files alone would call it unknown.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;

/// Process entries the deploy runs once, never as a logging service.
const ONE_SHOT_KINDS: [&str; 2] = ["release", "preflight"];

fn process_kinds(paths: &RikuPaths, app: &str) -> Vec<String> {
    match crate::config::app_config::load_processes(paths, app) {
        Ok(Some(entries)) => entries
            .into_keys()
            .filter(|kind| !ONE_SHOT_KINDS.contains(&kind.as_str()))
            .collect(),
        Ok(None) => Vec::new(),
        Err(e) => {
            tracing::warn!("cannot read the process list of '{app}': {e:#}");
            Vec::new()
        }
    }
}

fn compose_services(paths: &RikuPaths, app: &str) -> Vec<String> {
    let listed = match crate::deploy::containers::runs_containers(app, paths) {
        Ok(true) => crate::deploy::containers::service_names(app, paths),
        Ok(false) => return Vec::new(),
        Err(e) => Err(e),
    };
    match listed {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!("cannot list the container services of '{app}': {e}");
            Vec::new()
        }
    }
}

/// Service names from `{service}.{n}.log` files in an app's log directory.
fn log_file_services(app_log_dir: &Path) -> BTreeSet<String> {
    let mut services = BTreeSet::new();
    if let Ok(entries) = fs::read_dir(app_log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".log") && name != "deploy.log" {
                let stem = &name[..name.len() - 4];
                if let Some(pos) = stem.rfind('.') {
                    let svc = &stem[..pos];
                    if !svc.is_empty() {
                        services.insert(svc.to_string());
                    }
                }
            }
        }
    }
    services
}

/// Every service name of `app`, sorted.
pub(super) fn app_services(paths: &RikuPaths, app: &str) -> BTreeSet<String> {
    let mut services = log_file_services(&paths.log_root.join(app));
    services.extend(process_kinds(paths, app));
    services.extend(compose_services(paths, app));
    services
}

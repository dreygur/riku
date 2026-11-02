//! Log tailing commands for deployed apps.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid, validate_app_name};

mod tail;

use tail::{multi_tail, tail_deploy_log};

/// Show the persistent deploy log for an app.
#[allow(clippy::question_mark)]
pub fn cmd_deploy_logs(paths: &RikuPaths, app: &str, follow: bool) -> Result<()> {
    let app = match validate_app_name(app) {
        Ok(a) => a,
        Err(e) => return Err(e),
    };
    let log_file = paths.deploy_log_file(&app);

    if !log_file.exists() {
        display::warn(&format!(
            "No deploy log found for '{app}'. Deploy the app first."
        ));
        return Ok(());
    }

    display::section(&format!("Deploy log: {app}"));

    if follow {
        tail_deploy_log(&log_file)
    } else {
        let content = match fs::read_to_string(&log_file) {
            Ok(c) => c,
            Err(e) => return Err(e.into()),
        };
        for line in content.lines() {
            println!("{line}");
        }
        Ok(())
    }
}

/// Tail all process log files for an app or a single service.
#[allow(clippy::question_mark)]
pub fn cmd_logs(paths: &RikuPaths, app: &str, service: &str) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(e) => return Err(e),
    };
    let app_log_dir = paths.log_root.join(&app);

    if !service.is_empty() && service != "*" {
        let pattern = app_log_dir.join(format!("{service}.*.log"));
        let logfiles = match glob::glob(pattern.to_str().unwrap_or("")) {
            Ok(g) => g.filter_map(Result::ok).collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        if logfiles.is_empty() {
            let known = known_services(&app_log_dir);
            if known.is_empty() {
                display::warn(&format!("No logs found for app '{app}'."));
                return Ok(());
            }
            return Err(anyhow!(
                "unknown service '{service}' for app '{app}'; known services: {}",
                known.join(", ")
            ));
        }

        let str_files = logfiles
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        return multi_tail(&str_files);
    }

    let pattern = app_log_dir.join("*.log");
    let logfiles = match glob::glob(pattern.to_str().unwrap_or("")) {
        Ok(g) => g
            .filter_map(Result::ok)
            .filter(|p| p.file_name().is_some_and(|n| n != "deploy.log"))
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    if logfiles.is_empty() {
        display::warn(&format!("No logs found for app '{app}'."));
    } else {
        match multi_tail(&logfiles) {
            Ok(()) => (),
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Find all known service names from `{service}.{n}.log` files in an app's log directory.
pub(crate) fn known_services(app_log_dir: &Path) -> Vec<String> {
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
    services.into_iter().collect()
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;

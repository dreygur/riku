//! Log tailing commands for deployed apps.

use std::fs;

use anyhow::{anyhow, Result};
use riku_container::types::ServiceName;

use crate::config::RikuPaths;
use crate::util::{display, exit_if_invalid, validate_app_name};

mod services;
mod tail;

use services::app_services;
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

    display::step_done(&format!("Deploy log: {app}"), "");

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

/// The service `riku logs` was asked for: `None` for every service (empty or
/// `*`), otherwise a validated name. The name goes into a glob pattern, so
/// anything that is not a plain service name, such as `../other` or `a*`, is
/// refused here rather than matched against other apps' files.
fn requested_service(service: &str) -> Result<Option<ServiceName>> {
    if service.is_empty() || service == "*" {
        return Ok(None);
    }
    match ServiceName::try_from(service) {
        Ok(name) => Ok(Some(name)),
        Err(e) => Err(e),
    }
}

/// Tail all process log files for an app or a single service.
#[allow(clippy::question_mark)]
pub fn cmd_logs(paths: &RikuPaths, app: &str, service: &str) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(e) => return Err(e),
    };
    let service = match requested_service(service) {
        Ok(service) => service,
        Err(e) => return Err(e),
    };
    let app_log_dir = paths.log_root.join(&app);

    if let Some(service) = service {
        let pattern = app_log_dir.join(format!("{service}.*.log"));
        let logfiles = match glob::glob(pattern.to_str().unwrap_or("")) {
            Ok(g) => g.filter_map(Result::ok).collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        if logfiles.is_empty() {
            let known = app_services(paths, &app);
            if known.contains(service.as_str()) {
                display::warn(&format!(
                    "'{service}' of '{app}' has not written any output yet."
                ));
                return Ok(());
            }
            if known.is_empty() {
                display::warn(&format!("No logs found for app '{app}'."));
                return Ok(());
            }
            let known: Vec<String> = known.into_iter().collect();
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

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;

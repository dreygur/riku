//! Display stopped app scaling configuration from the SCALING file.

use std::fs;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::display;

#[allow(clippy::question_mark)]
pub(super) fn show_desired_scale_verbose(paths: &RikuPaths, app: &str) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = match fs::read_to_string(&config_file) {
            Ok(c) => c,
            Err(e) => return Err(e.into()),
        };
        let headers = vec!["KIND", "DESIRED", "STATUS"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                if let Some(pos) = line.find('=') {
                    let kind = line[..pos].trim().to_string();
                    let count = line[pos + 1..].trim().to_string();
                    rows.push(vec![kind, count, "stopped".to_string()]);
                }
            }
        }

        if !rows.is_empty() {
            crate::util::print_table_with_title(
                &format!("=== Processes for '{app}' (stopped) ==="),
                &headers,
                &rows,
                2,
            );
            return Ok(());
        }
    }

    display::warn(&format!("No processes configured for app '{app}'."));
    Ok(())
}

#[allow(clippy::question_mark)]
pub(super) fn show_scaling_compact(paths: &RikuPaths, app: &str) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = match fs::read_to_string(&config_file) {
            Ok(c) => c,
            Err(e) => return Err(e.into()),
        };
        let headers = vec!["KIND", "DESIRED"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                if let Some(pos) = line.find('=') {
                    let kind = line[..pos].trim().to_string();
                    let count = line[pos + 1..].trim().to_string();
                    rows.push(vec![kind, count]);
                }
            }
        }

        if !rows.is_empty() {
            crate::util::print_table_with_title(
                &format!("=== Scaling for '{app}' (stopped) ==="),
                &headers,
                &rows,
                2,
            );
            return Ok(());
        }
    }

    display::warn(&format!("No running processes found for app '{app}'."));
    Ok(())
}

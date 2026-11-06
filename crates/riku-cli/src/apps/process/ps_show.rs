use std::collections::HashMap;
use std::fs;

use anyhow::Result;

use riku_supervisor::config::app_worker_configs;

use super::helpers::{load_stats, lookup_process_stats, read_worker_config};
use super::scaling_display::{show_desired_scale_verbose, show_scaling_compact};
use crate::config::RikuPaths;
use crate::util::exit_if_invalid;

const CONTAINER_HEADERS: &[&str] = &[
    "SERVICE",
    "STATUS",
    "MEMORY",
    "CPU",
    "RESTARTS",
    "CONTAINER ID",
    "IMAGE",
];

/// Build the container table (headers and rows) for an app from the supervisor stats file.
///
/// Returns `None` when the app has no container-backed processes, so the caller
/// falls back to the Procfile rows.
#[allow(clippy::question_mark)]
pub(crate) fn container_rows_for_app(
    paths: &RikuPaths,
    app: &str,
) -> Option<(Vec<&'static str>, Vec<Vec<String>>)> {
    let stats_data = match load_stats(paths) {
        Some(s) => s,
        None => return None,
    };
    let app_stats = match stats_data.into_iter().find(|a| a.app == app) {
        Some(a) => a,
        None => return None,
    };

    let container_procs: Vec<_> = app_stats
        .processes
        .into_iter()
        .filter_map(|p| {
            p.container
                .map(|c| (p.kind, p.ordinal, p.restart_count, p.memory_bytes, c))
        })
        .collect();

    if container_procs.is_empty() {
        return None;
    }

    let mut rows = Vec::new();
    for (kind, ordinal, restarts, memory_bytes, c) in container_procs {
        let service_n = format!("{kind}.{ordinal}");
        let state_detail = match c.exit_code {
            Some(code) => format!("{} {code}", c.state),
            None => c.state,
        };
        let memory = format!("{:.1} MB", memory_bytes as f64 / 1024.0 / 1024.0);
        let cpu = format!("{:.1}%", c.cpu_percent);
        let container_id = if c.id.len() > 12 {
            c.id[..12].to_string()
        } else {
            c.id
        };
        rows.push(vec![
            service_n,
            state_detail,
            memory,
            cpu,
            restarts.to_string(),
            container_id,
            c.image,
        ]);
    }

    Some((CONTAINER_HEADERS.to_vec(), rows))
}

/// Show process scaling info.
#[allow(clippy::question_mark)]
pub fn cmd_ps_show(paths: &RikuPaths, app: &str, verbose: bool) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(e) => return Err(e),
    };

    if let Some((headers, rows)) = container_rows_for_app(paths, &app) {
        crate::util::print_table_with_title(
            &format!("=== Processes for '{app}' ==="),
            &headers,
            &rows,
            2,
        );
        return Ok(());
    }

    let worker_configs = app_worker_configs(&paths.workers_enabled, &app);

    if verbose && worker_configs.is_empty() {
        return show_desired_scale_verbose(paths, &app);
    }

    if worker_configs.is_empty() {
        return show_scaling_compact(paths, &app);
    }

    if verbose {
        show_running_verbose(paths, &app, worker_configs)
    } else {
        show_running_compact(paths, &app, &worker_configs)
    }
}

fn show_running_verbose(
    paths: &RikuPaths,
    app: &str,
    worker_configs: Vec<std::path::PathBuf>,
) -> Result<()> {
    let headers = vec!["PROCESS", "KIND", "PID", "STATUS", "HEALTH"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let stats_data = load_stats(paths);

    for config_path in worker_configs {
        let worker_config = match read_worker_config(&config_path) {
            Some(config) => config,
            None => continue,
        };

        let process_name = worker_config.worker.name().to_string();
        let (pid, status, health) = lookup_process_stats(&stats_data, &process_name);
        rows.push(vec![
            process_name,
            worker_config.worker.kind,
            pid,
            status,
            health,
        ]);
    }

    crate::util::print_table_with_title(
        &format!("=== Processes for '{app}' ==="),
        &headers,
        &rows,
        2,
    );
    Ok(())
}

#[allow(clippy::question_mark)]
fn show_running_compact(
    paths: &RikuPaths,
    app: &str,
    worker_configs: &[std::path::PathBuf],
) -> Result<()> {
    let headers = vec!["KIND", "COUNT"];
    let mut rows: Vec<Vec<String>> = Vec::new();

    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = match fs::read_to_string(&config_file) {
            Ok(c) => c,
            Err(e) => return Err(e.into()),
        };
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
    } else {
        let mut counts: HashMap<String, u32> = HashMap::new();
        for config_path in worker_configs {
            if let Some(worker_config) = read_worker_config(config_path) {
                *counts.entry(worker_config.worker.kind).or_insert(0) += 1;
            }
        }
        for (kind, count) in counts {
            rows.push(vec![kind, count.to_string()]);
        }
    }

    crate::util::print_table_with_title(
        &format!("=== Scaling for '{app}' ==="),
        &headers,
        &rows,
        2,
    );
    Ok(())
}

#[cfg(test)]
#[path = "ps_show_tests.rs"]
mod tests;

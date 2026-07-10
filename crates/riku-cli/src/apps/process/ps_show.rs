use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::{echo, exit_if_invalid};

use super::ps_all::{collect_worker_configs, load_stats, lookup_process_stats, read_worker_config};

/// Show process scaling info.
pub fn cmd_ps_show(paths: &RikuPaths, app: &str, verbose: bool) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;
    let worker_configs = collect_worker_configs(paths, &app);

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

fn show_desired_scale_verbose(paths: &RikuPaths, app: &str) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
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
                &format!("=== Processes for '{}' (stopped) ===", app),
                &headers,
                &rows,
                2,
            );
            return Ok(());
        }
    }

    echo(
        &format!("No processes configured for app '{}'.", app),
        "yellow",
    );
    Ok(())
}

fn show_scaling_compact(paths: &RikuPaths, app: &str) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
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
                &format!("=== Scaling for '{}' (stopped) ===", app),
                &headers,
                &rows,
                2,
            );
            return Ok(());
        }
    }

    echo(
        &format!("No running processes found for app '{}'.", app),
        "yellow",
    );
    Ok(())
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

        let process_name = format!(
            "{}-{}-{}",
            worker_config.worker.app, worker_config.worker.kind, worker_config.worker.ordinal
        );
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
        &format!("=== Processes for '{}' ===", app),
        &headers,
        &rows,
        2,
    );
    Ok(())
}

fn show_running_compact(
    paths: &RikuPaths,
    app: &str,
    worker_configs: &[std::path::PathBuf],
) -> Result<()> {
    let headers = vec!["KIND", "COUNT"];
    let mut rows: Vec<Vec<String>> = Vec::new();

    let config_file = paths.env_root.join(app).join("SCALING");
    if config_file.exists() {
        let content = fs::read_to_string(&config_file)?;
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
        &format!("=== Scaling for '{}' ===", app),
        &headers,
        &rows,
        2,
    );
    Ok(())
}

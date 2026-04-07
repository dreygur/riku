/// Agent commands for runtime operations: logs, ps, restart, stop, run, stats.
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::config::RikuPaths;
use crate::util::exit_if_invalid;

use crate::cli::agent::types::AgentResponse;

pub fn cmd_agent_logs(paths: &RikuPaths, app: &str, process: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let log_dir = paths.log_root.join(&app);
    let mut lines: Vec<String> = Vec::new();

    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "log").unwrap_or(false)
                    && (process == "*"
                        || path
                            .file_stem()
                            .map(|s| s.to_string_lossy().contains(process))
                            .unwrap_or(false))
                {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for line in content.lines().take(100) {
                            lines.push(line.to_string());
                        }
                    }
                }
            }
        }
    }

    AgentResponse::success(json!({"app": app, "lines": lines}))
}

pub fn cmd_agent_ps(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let pattern = paths.workers_enabled.join(format!("{}-*.toml", app));
    let worker_count = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);

    AgentResponse::success(json!({
        "app": app,
        "workers": worker_count,
        "running": worker_count > 0
    }))
}

pub fn cmd_agent_restart(paths: &RikuPaths, app: &str, _process: Option<&str>) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    match crate::cli::apps::cmd_restart(paths, &app) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Restart completed for {}", app)
        })),
        Err(e) => AgentResponse::error("RESTART_FAILED", &format!("Restart failed: {}", e)),
    }
}

pub fn cmd_agent_stop(paths: &RikuPaths, app: &str) -> AgentResponse {
    let _app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let token = format!(
        "stop-{}-{}",
        app,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    AgentResponse::confirmation_required("stop", app, &token)
}

pub fn cmd_agent_stop_confirm(paths: &RikuPaths, app: &str, _token: &str) -> AgentResponse {
    match crate::cli::apps::cmd_stop(paths, app) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Application '{}' stopped successfully", app)
        })),
        Err(e) => AgentResponse::error("STOP_FAILED", &format!("Stop failed: {}", e)),
    }
}

pub fn cmd_agent_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let command_str = cmd.join(" ");
    AgentResponse::success(json!({
        "app": app,
        "command": command_str,
        "message": "Command execution not yet implemented"
    }))
}

pub fn cmd_agent_stats(paths: &RikuPaths, app: Option<&str>) -> AgentResponse {
    match app {
        Some(app_name) => {
            let app = match exit_if_invalid(app_name, &paths.app_root) {
                Ok(a) => a,
                Err(_) => {
                    return AgentResponse::error(
                        "APP_NOT_FOUND",
                        &format!("Application '{}' not found", app_name),
                    )
                }
            };

            AgentResponse::success(json!({
                "app": app,
                "cpu": 0.0,
                "memory": 0,
                "requests": 0
            }))
        }
        None => {
            let mut stats = HashMap::new();
            if let Ok(entries) = fs::read_dir(&paths.app_root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    stats.insert(name, json!({"cpu": 0.0, "memory": 0, "requests": 0}));
                }
            }

            AgentResponse::success(json!({"apps": stats}))
        }
    }
}

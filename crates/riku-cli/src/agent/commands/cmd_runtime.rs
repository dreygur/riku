/// Agent commands for runtime operations: logs, ps, restart, stop, run, stats.
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use riku_supervisor::config::app_worker_configs;
use serde_json::json;

use crate::config::RikuPaths;
use crate::util::existing_app;

use crate::agent::confirm;
use crate::agent::types::AgentResponse;

/// Lines `agent logs` returns from each log file, newest last.
const LOG_LINES: usize = 100;

/// Streams the file so memory stays at `count` lines however large the log
/// grows. Bytes that are not UTF-8 are replaced rather than dropping the file.
fn last_lines(path: &Path, count: usize) -> Vec<String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let mut reader = BufReader::new(file);
    let mut kept: VecDeque<String> = VecDeque::with_capacity(count);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if kept.len() == count {
            kept.pop_front();
        }
        let text = String::from_utf8_lossy(&line);
        kept.push_back(text.trim_end_matches(['\n', '\r']).to_string());
    }
    kept.into()
}

pub fn cmd_agent_logs(paths: &RikuPaths, app: &str, process: &str) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
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
                    lines.extend(last_lines(&path, LOG_LINES));
                }
            }
        }
    }

    AgentResponse::success(json!({"app": app, "lines": lines}))
}

pub fn cmd_agent_ps(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let worker_count = app_worker_configs(&paths.workers_enabled, &app).len();

    AgentResponse::success(json!({
        "app": app,
        "workers": worker_count,
        "running": worker_count > 0
    }))
}

pub fn cmd_agent_restart(paths: &RikuPaths, app: &str, service: Option<&str>) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    match crate::apps::cmd_restart(paths, &app, service) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Restart completed for {}", app)
        })),
        Err(e) => AgentResponse::error("RESTART_FAILED", &format!("Restart failed: {}", e)),
    }
}

pub fn cmd_agent_stop(paths: &RikuPaths, state: &Path, app: &str) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };
    confirm::request(state, "stop", &app)
}

pub fn cmd_agent_stop_confirm(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    token: &str,
) -> AgentResponse {
    let app = match existing_app(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };
    if let Some(refusal) = confirm::refused(state, "stop", &app, token) {
        return refusal;
    }

    match crate::apps::cmd_stop(paths, &app, None) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Application '{}' stopped successfully", app)
        })),
        Err(e) => AgentResponse::error("STOP_FAILED", &format!("Stop failed: {}", e)),
    }
}

pub fn cmd_agent_stats(paths: &RikuPaths, app: Option<&str>) -> AgentResponse {
    match app {
        Some(app_name) => {
            let app = match existing_app(app_name, &paths.app_root) {
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

#[cfg(test)]
#[path = "cmd_runtime_tests.rs"]
mod tests;

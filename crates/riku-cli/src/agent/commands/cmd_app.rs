/// Agent commands for app lifecycle: apps, deploy, destroy.
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use riku_supervisor::config::app_worker_configs;
use serde_json::json;

use crate::config::RikuPaths;
use crate::util::exit_if_invalid;

use crate::agent::confirm;
use crate::agent::types::AgentResponse;

pub fn cmd_agent_apps(paths: &RikuPaths) -> AgentResponse {
    let app_root = &paths.app_root;
    if !app_root.exists() {
        return AgentResponse::success(json!([]));
    }

    let apps: Vec<serde_json::Value> = fs::read_dir(app_root)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let running = check_app_running(paths, &name);
                    json!({"name": name, "running": running})
                })
                .collect()
        })
        .unwrap_or_default();

    AgentResponse::success(json!({"apps": apps}))
}

fn check_app_running(paths: &RikuPaths, app: &str) -> bool {
    !app_worker_configs(&paths.workers_enabled, app).is_empty()
}

pub fn cmd_agent_deploy(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return AgentResponse::error("APP_NOT_FOUND", &format!("Application '{}' not found", app));
    }

    let job_id = format!(
        "deploy-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    match crate::apps::cmd_deploy(paths, app, None) {
        Ok(_) => AgentResponse::success(json!({
            "job_id": job_id,
            "status": "completed",
            "message": format!("Deployment completed for {}", app)
        })),
        Err(e) => AgentResponse::error("DEPLOY_FAILED", &format!("Deployment failed: {}", e)),
    }
}

fn not_found(app: &str) -> AgentResponse {
    AgentResponse::error("APP_NOT_FOUND", &format!("Application '{}' not found", app))
}

pub fn cmd_agent_destroy_request(paths: &RikuPaths, state: &Path, app: &str) -> AgentResponse {
    match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => confirm::request(state, "destroy", &app),
        Err(_) => not_found(app),
    }
}

pub fn cmd_agent_destroy_confirm(
    paths: &RikuPaths,
    state: &Path,
    app: &str,
    token: &str,
) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(app) => app,
        Err(_) => return not_found(app),
    };
    if let Some(refusal) = confirm::refused(state, "destroy", &app, token) {
        return refusal;
    }

    match crate::apps::cmd_destroy(paths, &app, crate::apps::DestroyMode::Safe) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Application '{}' destroyed successfully", app)
        })),
        Err(e) => AgentResponse::error("DESTROY_FAILED", &format!("Destroy failed: {}", e)),
    }
}

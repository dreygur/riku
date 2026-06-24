/// Agent commands for app lifecycle: apps, deploy, destroy.
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::config::RikuPaths;

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
    let ini_pattern = paths.workers_enabled.join(format!("{}-*.ini", app));
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));

    let ini_matches = glob::glob(ini_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);
    let toml_matches = glob::glob(toml_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);

    ini_matches + toml_matches > 0
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

pub fn cmd_agent_destroy_request(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return AgentResponse::error("APP_NOT_FOUND", &format!("Application '{}' not found", app));
    }

    let token = format!(
        "destroy-{}-{}",
        app,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let token_file = format!("/tmp/riku-confirm-{}", token);
    let _ = fs::write(&token_file, format!("destroy:{}:{}", app, token));

    AgentResponse::confirmation_required("destroy", app, &token)
}

pub fn cmd_agent_destroy_confirm(paths: &RikuPaths, app: &str, token: &str) -> AgentResponse {
    let token_file = format!("/tmp/riku-confirm-{}", token);
    if let Ok(content) = fs::read_to_string(&token_file) {
        if content.starts_with(&format!("destroy:{}:", app)) {
            let _ = fs::remove_file(&token_file);

            match crate::apps::cmd_destroy(paths, app) {
                Ok(_) => AgentResponse::success(json!({
                    "message": format!("Application '{}' destroyed successfully", app)
                })),
                Err(e) => AgentResponse::error("DESTROY_FAILED", &format!("Destroy failed: {}", e)),
            }
        } else {
            AgentResponse::error("INVALID_TOKEN", "Token does not match app")
        }
    } else {
        AgentResponse::error("INVALID_TOKEN", "Invalid or expired confirmation token")
    }
}

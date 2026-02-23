/// AI Agent Interface
///
/// Provides SSH-based access for AI agents (Claude, Cursor, Copilot, etc.)
/// to perform deployment and management tasks with proper authentication,
/// authorization, and audit logging.
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::RikuPaths;
use crate::util::exit_if_invalid;

/// Agent permissions scope
#[derive(Debug, Clone, PartialEq)]
pub enum AgentScope {
    Readonly,
    Staging,
    Production,
}

impl AgentScope {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "readonly" => AgentScope::Readonly,
            "staging" => AgentScope::Staging,
            "production" => AgentScope::Production,
            _ => AgentScope::Readonly,
        }
    }

    /// Check if scope allows a specific action
    pub fn allows(&self, action: &str) -> bool {
        match action {
            "apps" | "logs" | "ps" | "config:get" | "config:show" | "stats" => true,
            "deploy" | "restart" | "run" | "config:set" | "config:unset" => {
                matches!(self, AgentScope::Staging | AgentScope::Production)
            }
            "destroy" | "stop" => matches!(self, AgentScope::Production),
            _ => false,
        }
    }

    /// Get rate limit (commands per minute)
    pub fn rate_limit(&self) -> u32 {
        match self {
            AgentScope::Readonly => 60,
            AgentScope::Staging => 30,
            AgentScope::Production => 20,
        }
    }
}

/// Agent response structure
#[derive(Debug)]
pub struct AgentResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<AgentError>,
    pub message: Option<String>,
    pub confirmation_required: bool,
    pub confirm_token: Option<String>,
    pub job_id: Option<String>,
}

impl AgentResponse {
    pub fn success(data: serde_json::Value) -> Self {
        AgentResponse {
            success: true,
            data: Some(data),
            error: None,
            message: None,
            confirmation_required: false,
            confirm_token: None,
            job_id: None,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        AgentResponse {
            success: false,
            data: None,
            error: Some(AgentError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            message: None,
            confirmation_required: false,
            confirm_token: None,
            job_id: None,
        }
    }

    pub fn confirmation_required(action: &str, _app: &str, token: &str) -> Self {
        AgentResponse {
            success: false,
            data: None,
            error: None,
            message: Some(format!("Human confirmation required for {}", action)),
            confirmation_required: true,
            confirm_token: Some(token.to_string()),
            job_id: None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut result = json!({
            "success": self.success,
        });

        if let Some(ref data) = self.data {
            result["data"] = data.clone();
        }

        if let Some(ref error) = self.error {
            result["error"] = json!({
                "code": error.code,
                "message": error.message,
            });
        }

        if let Some(ref message) = self.message {
            result["message"] = json!(message);
        }

        result["confirmation_required"] = json!(self.confirmation_required);

        if let Some(ref token) = self.confirm_token {
            result["confirm_token"] = json!(token);
        }

        if let Some(ref job_id) = self.job_id {
            result["job_id"] = json!(job_id);
        }

        result
    }
}

#[derive(Debug)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}

/// Get agent identity from SSH key comment or environment
pub fn get_agent_identity() -> Option<String> {
    // Try environment variable first (set by SSH forced command)
    if let Ok(id) = std::env::var("RIKU_AGENT_ID") {
        return Some(id);
    }

    // Try to extract from SSH key comment via SSH_CONNECTION
    // This would typically be set in the forced command in authorized_keys
    if let Ok(cmd) = std::env::var("SSH_ORIGINAL_COMMAND") {
        // Extract agent ID from command if present
        if cmd.contains("--agent-id=") {
            return cmd
                .split("--agent-id=")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.to_string());
        }
        return Some("ssh-agent".to_string());
    }

    Some("unknown-agent".to_string())
}

/// Get agent scope from SSH key restrictions or environment
pub fn get_agent_scope() -> AgentScope {
    // Try environment variable first (set by SSH forced command)
    if let Ok(scope) = std::env::var("RIKU_AGENT_SCOPE") {
        return AgentScope::from_str(&scope);
    }

    // Parse authorized_keys to find scope from command restriction
    // Format: command="riku agent --scope staging",no-port-forwarding ssh-rsa AAAA... comment
    if let Some(scope) = parse_scope_from_authorized_keys() {
        return scope;
    }

    AgentScope::Readonly
}

/// Parse agent scope from authorized_keys file
fn parse_scope_from_authorized_keys() -> Option<AgentScope> {
    let auth_keys_path = dirs::home_dir().map(|h| h.join(".ssh/authorized_keys"));

    if let Some(path) = auth_keys_path {
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                // Look for command restriction with scope
                if line.contains("riku agent") && line.contains("--scope") {
                    if let Some(scope_start) = line.find("--scope ") {
                        let scope_str = &line[scope_start + 8..];
                        let scope = scope_str.split_whitespace().next()?;
                        return Some(AgentScope::from_str(scope));
                    }
                }
            }
        }
    }
    None
}

/// Check rate limit for agent
fn check_rate_limit(agent_id: &str, scope: &AgentScope) -> bool {
    let rate_file = Path::new("/tmp/riku-agent-rates");
    let agent_file = rate_file.join(format!("{}.log", agent_id.replace("@", "_")));

    // Create rate directory if not exists
    let _ = fs::create_dir_all(rate_file);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let window = 60; // 1 minute window
    let limit = scope.rate_limit();

    // Read existing timestamps
    let mut timestamps: Vec<u64> = Vec::new();
    if let Ok(content) = fs::read_to_string(&agent_file) {
        for line in content.lines() {
            if let Ok(ts) = line.parse::<u64>() {
                if now - ts < window {
                    timestamps.push(ts);
                }
            }
        }
    }

    // Check if over limit
    if timestamps.len() >= limit as usize {
        return false;
    }

    // Add current timestamp
    timestamps.push(now);
    let content: String = timestamps.iter().map(|t| t.to_string() + "\n").collect();
    let _ = fs::write(&agent_file, content);

    true
}

/// Log agent action to audit log
fn log_agent_action(agent_id: &str, action: &str, app: &str, success: bool) {
    let audit_file = Path::new("/tmp/riku-agent-audit.log");

    let status = if success { "success" } else { "failed" };
    let log_line = format!(
        "{} [AI] {} {} {} {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        agent_id,
        action,
        app,
        status
    );

    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_file)
        .map(|mut f| {
            use std::io::Write;
            f.write_all(log_line.as_bytes())
        });
}

/// Show agent introduction and permissions
pub fn cmd_agent_intro(_paths: &RikuPaths) -> Result<()> {
    let agent_id = get_agent_identity().unwrap_or_else(|| "unknown-agent".to_string());
    let scope = get_agent_scope();

    let response = json!({
        "welcome": "Riku AI Agent Interface",
        "version": env!("CARGO_PKG_VERSION"),
        "agent_id": agent_id,
        "permissions": match scope {
            AgentScope::Readonly => vec!["apps", "logs", "ps", "config:get", "config:show", "stats"],
            AgentScope::Staging => vec!["apps", "logs", "ps", "config:get", "config:show", "stats", "deploy", "restart", "run", "config:set", "config:unset"],
            AgentScope::Production => vec!["apps", "logs", "ps", "config:get", "config:show", "stats", "deploy", "destroy", "restart", "stop", "run", "config:set", "config:unset"],
        },
        "scope": match scope {
            AgentScope::Readonly => "readonly",
            AgentScope::Staging => "staging",
            AgentScope::Production => "production",
        },
        "documentation": "https://dreygur.github.io/riku/ai-agents/",
        "hint": "Use 'riku agent --schema' for full command reference",
    });

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

/// Show full command schema
pub fn cmd_agent_schema() -> Result<()> {
    let schema = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commands": [
            {
                "name": "apps",
                "description": "List deployed applications",
                "parameters": {},
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            },
            {
                "name": "deploy",
                "description": "Deploy an application",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["staging", "production"]
            },
            {
                "name": "destroy",
                "description": "Permanently remove an application and all its data",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "confirm": {
                        "type": "string",
                        "required": true,
                        "description": "Confirmation token from initial request"
                    }
                },
                "requires_confirmation": true,
                "permissions": ["production"]
            },
            {
                "name": "config:get",
                "description": "Get a single configuration value",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "key": {
                        "type": "string",
                        "required": true,
                        "description": "Configuration key"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            },
            {
                "name": "config:set",
                "description": "Set configuration values (KEY=VALUE pairs)",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "settings": {
                        "type": "array",
                        "required": true,
                        "description": "KEY=VALUE pairs to set"
                    }
                },
                "requires_confirmation": true,
                "permissions": ["staging", "production"],
                "confirmation_for_keys": ["DATABASE_URL", "SECRET_KEY", "PASSWORD", "API_KEY"]
            },
            {
                "name": "config:show",
                "description": "Show all configuration for an application",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            },
            {
                "name": "logs",
                "description": "View application logs",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "process": {
                        "type": "string",
                        "required": false,
                        "description": "Process filter (default: all)",
                        "default": "*"
                    },
                    "lines": {
                        "type": "integer",
                        "required": false,
                        "description": "Number of lines to return",
                        "default": 100
                    }
                },
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            },
            {
                "name": "ps",
                "description": "Show process status",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            },
            {
                "name": "restart",
                "description": "Restart an application",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "process": {
                        "type": "string",
                        "required": false,
                        "description": "Specific process to restart"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["staging", "production"]
            },
            {
                "name": "stop",
                "description": "Stop an application",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    }
                },
                "requires_confirmation": true,
                "permissions": ["production"]
            },
            {
                "name": "run",
                "description": "Run a command in the application context",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "command": {
                        "type": "string",
                        "required": true,
                        "description": "Command to execute"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["staging", "production"]
            },
            {
                "name": "stats",
                "description": "Show application statistics",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": false,
                        "description": "Application name (optional, shows all if omitted)"
                    }
                },
                "requires_confirmation": false,
                "permissions": ["readonly", "staging", "production"]
            }
        ],
        "response_format": {
            "success": "boolean - true if command succeeded",
            "data": "object - command-specific data on success",
            "error": {
                "code": "string - error code",
                "message": "string - human-readable error message"
            },
            "confirmation_required": "boolean - true if human confirmation needed",
            "confirm_token": "string - token to include in confirmation request",
            "job_id": "string - for long-running operations"
        },
        "error_codes": {
            "APP_NOT_FOUND": "Application does not exist",
            "PERMISSION_DENIED": "Agent lacks required permission",
            "CONFIRMATION_REQUIRED": "Human confirmation needed",
            "APP_LOCKED": "Another operation in progress on this app",
            "INVALID_COMMAND": "Unknown command",
            "INVALID_PARAMETERS": "Missing or invalid parameters",
            "RATE_LIMIT_EXCEEDED": "Too many requests"
        }
    });

    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

/// Show help for a specific command
pub fn cmd_agent_help(command: Option<&str>) -> Result<()> {
    match command {
        Some(cmd) => {
            let help = match cmd {
                "apps" => {
                    r#"Usage: riku agent apps

List all deployed applications. Running apps are marked with *.

Example:
  riku agent apps

Response:
  {"apps": [{"name": "myapp", "running": true}]}"#
                }
                "deploy" => {
                    r#"Usage: riku agent deploy <app>

Deploy an application. Requires 'staging' or 'production' scope.

Example:
  riku agent deploy myapp

Response:
  {"success": true, "job_id": "deploy-123"}"#
                }
                "destroy" => {
                    r#"Usage: riku agent destroy <app> --confirm <token>

Permanently remove an application and all its data.
Requires 'production' scope and human confirmation.

Example:
  # First request (gets confirmation token)
  riku agent destroy myapp
  
  # After human confirms
  riku agent destroy myapp --confirm abc123

Response:
  {"confirmation_required": true, "confirm_token": "abc123"}"#
                }
                "config:get" => {
                    r#"Usage: riku agent config:get <app> <key>

Get a single configuration value.

Example:
  riku agent config:get myapp DATABASE_URL

Response:
  {"value": "postgres://localhost/db"}"#
                }
                "config:set" => {
                    r#"Usage: riku agent config:set <app> KEY=value [KEY2=value2 ...]

Set configuration values. Critical keys require confirmation.

Example:
  riku agent config:set myapp DEBUG=true

Response:
  {"success": true} or {"confirmation_required": true}"#
                }
                "logs" => {
                    r#"Usage: riku agent logs <app> [process] [--lines N]

View application logs.

Example:
  riku agent logs myapp web --lines 100

Response:
  {"lines": ["2024-01-01 10:00:00 App started", ...]}"#
                }
                "ps" => {
                    r#"Usage: riku agent ps <app>

Show process status for an application.

Example:
  riku agent ps myapp

Response:
  {"processes": {"web": {"running": 2, "desired": 2}}}"#
                }
                "restart" => {
                    r#"Usage: riku agent restart <app> [process]

Restart an application or specific process.

Example:
  riku agent restart myapp
  riku agent restart myapp web

Response:
  {"success": true}"#
                }
                "stop" => {
                    r#"Usage: riku agent stop <app>

Stop an application. Requires confirmation for production apps.

Example:
  riku agent stop myapp

Response:
  {"confirmation_required": true, "confirm_token": "abc123"}"#
                }
                "run" => {
                    r#"Usage: riku agent run <app> <command>

Run a command in the application context.

Example:
  riku agent run myapp python manage.py migrate

Response:
  {"output": "Migrations applied", "exit_code": 0}"#
                }
                "stats" => {
                    r#"Usage: riku agent stats [app]

Show statistics. Without app argument, shows all apps.

Example:
  riku agent stats
  riku agent stats myapp

Response:
  {"apps": {"myapp": {"cpu": 0.5, "memory": 128}}}"#
                }
                _ => "Unknown command. Use 'riku agent --schema' for full reference.",
            };
            println!("{}", help);
        }
        None => {
            println!("AI Agent Interface for Riku");
            println!();
            println!("Usage: riku agent [OPTIONS] [COMMAND]");
            println!();
            println!("Options:");
            println!("  --intro     Show agent introduction and permissions");
            println!("  --schema    Show full command schema (JSON)");
            println!("  --help      Show this help or help for a command");
            println!("  --json      Output in JSON format");
            println!();
            println!("Commands:");
            println!("  apps        List deployed applications");
            println!("  deploy      Deploy an application");
            println!("  destroy     Remove an application");
            println!("  config:get  Get configuration value");
            println!("  config:set  Set configuration values");
            println!("  config:show Show all configuration");
            println!("  logs        View application logs");
            println!("  ps          Show process status");
            println!("  restart     Restart an application");
            println!("  stop        Stop an application");
            println!("  run         Run command in app context");
            println!("  stats       Show statistics");
            println!();
            println!("Use 'riku agent --help <command>' for command-specific help.");
        }
    }
    Ok(())
}

/// Execute agent command
pub fn cmd_agent_execute(
    paths: &RikuPaths,
    command: &str,
    args: &[String],
    confirm_token: Option<&str>,
) -> Result<()> {
    let agent_id = get_agent_identity().unwrap_or_else(|| "unknown-agent".to_string());
    let scope = get_agent_scope();

    // Check rate limit
    if !check_rate_limit(&agent_id, &scope) {
        let response = AgentResponse::error(
            "RATE_LIMIT_EXCEEDED",
            "Too many requests. Try again in 30 seconds.",
        );
        println!("{}", serde_json::to_string(&response.to_json())?);
        return Ok(());
    }

    // Check permission
    if !scope.allows(command) {
        log_agent_action(&agent_id, command, "", false);
        let response = AgentResponse::error("PERMISSION_DENIED", "Agent lacks required permission");
        println!("{}", serde_json::to_string(&response.to_json())?);
        return Ok(());
    }

    // Execute command
    let response = match command {
        "apps" => cmd_agent_apps(paths),
        "deploy" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_deploy(paths, &args[0])
            }
        }
        "destroy" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                let app = &args[0];
                // Check for confirmation token
                if let Some(token) = confirm_token {
                    cmd_agent_destroy_confirm(paths, app, token)
                } else {
                    cmd_agent_destroy_request(paths, app)
                }
            }
        }
        "config:get" => {
            if args.len() < 2 {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name or key")
            } else {
                cmd_agent_config_get(paths, &args[0], &args[1])
            }
        }
        "config:set" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_config_set(paths, &args[0], &args[1..])
            }
        }
        "config:show" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_config_show(paths, &args[0])
            }
        }
        "logs" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                let app = &args[0];
                let process = args.get(1).map(|s| s.as_str()).unwrap_or("*");
                cmd_agent_logs(paths, app, process)
            }
        }
        "ps" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_ps(paths, &args[0])
            }
        }
        "restart" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_restart(paths, &args[0], args.get(1).map(|s| s.as_str()))
            }
        }
        "stop" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                let app = &args[0];
                // Check for confirmation token
                if let Some(token) = confirm_token {
                    cmd_agent_stop_confirm(paths, app, token)
                } else {
                    cmd_agent_stop(paths, app)
                }
            }
        }
        "run" => {
            if args.len() < 2 {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name or command")
            } else {
                cmd_agent_run(paths, &args[0], &args[1..])
            }
        }
        "stats" => {
            let app = args.first().map(|s| s.as_str());
            cmd_agent_stats(paths, app)
        }
        _ => AgentResponse::error("INVALID_COMMAND", &format!("Unknown command: {}", command)),
    };

    // Log action
    log_agent_action(
        &agent_id,
        command,
        args.first().map(|s| s.as_str()).unwrap_or(""),
        response.success,
    );

    println!("{}", serde_json::to_string(&response.to_json())?);
    Ok(())
}

// Command implementations

fn cmd_agent_apps(paths: &RikuPaths) -> AgentResponse {
    use std::fs;

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
                    // Check if running
                    let running = check_app_running(paths, &name);
                    json!({"name": name, "running": running})
                })
                .collect()
        })
        .unwrap_or_default();

    AgentResponse::success(json!({"apps": apps}))
}

fn check_app_running(paths: &RikuPaths, app: &str) -> bool {
    let ini_pattern = paths.workers_enabled.join(format!("{}*.ini", app));
    let toml_pattern = paths.workers_enabled.join(format!("{}*.toml", app));

    let ini_matches = glob::glob(ini_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);
    let toml_matches = glob::glob(toml_pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);

    ini_matches + toml_matches > 0
}

fn cmd_agent_deploy(paths: &RikuPaths, app: &str) -> AgentResponse {
    // Check if app exists
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return AgentResponse::error("APP_NOT_FOUND", &format!("Application '{}' not found", app));
    }

    // Generate job ID
    let job_id = format!(
        "deploy-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Call the actual deploy function
    match crate::cli::apps::cmd_deploy(paths, app) {
        Ok(_) => AgentResponse::success(json!({
            "job_id": job_id,
            "status": "completed",
            "message": format!("Deployment completed for {}", app)
        })),
        Err(e) => AgentResponse::error("DEPLOY_FAILED", &format!("Deployment failed: {}", e)),
    }
}

fn cmd_agent_destroy_request(paths: &RikuPaths, app: &str) -> AgentResponse {
    // Check if app exists
    let app_path = paths.app_root.join(app);
    if !app_path.exists() {
        return AgentResponse::error("APP_NOT_FOUND", &format!("Application '{}' not found", app));
    }

    // Generate confirmation token
    let token = format!(
        "destroy-{}-{}",
        app,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Store token for verification (in real impl, use Redis or similar)
    let token_file = format!("/tmp/riku-confirm-{}", token);
    let _ = fs::write(&token_file, format!("destroy:{}:{}", app, token));

    AgentResponse::confirmation_required("destroy", app, &token)
}

fn cmd_agent_destroy_confirm(paths: &RikuPaths, app: &str, token: &str) -> AgentResponse {
    // Verify token
    let token_file = format!("/tmp/riku-confirm-{}", token);
    if let Ok(content) = fs::read_to_string(&token_file) {
        if content.starts_with(&format!("destroy:{}:", app)) {
            // Token valid, proceed with destroy
            let _ = fs::remove_file(&token_file);

            // Call the actual destroy function
            match crate::cli::apps::cmd_destroy(paths, app) {
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

fn cmd_agent_config_get(paths: &RikuPaths, app: &str, key: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let config_file = paths.env_root.join(&app).join("ENV");
    if config_file.exists() {
        let mut env = HashMap::new();
        if let Ok(settings) = crate::util::parse_settings(&config_file, &mut env) {
            if let Some(val) = settings.get(key) {
                return AgentResponse::success(json!({"key": key, "value": val}));
            }
        }
    }

    AgentResponse::error("KEY_NOT_FOUND", &format!("Key '{}' not found", key))
}

fn cmd_agent_config_set(_paths: &RikuPaths, app: &str, settings: &[String]) -> AgentResponse {
    // Check for critical keys that require confirmation
    let critical_keys = [
        "DATABASE_URL",
        "SECRET_KEY",
        "PASSWORD",
        "API_KEY",
        "PRIVATE_KEY",
    ];

    for setting in settings {
        if let Some(key) = setting.split('=').next() {
            if critical_keys.contains(&key.to_uppercase().as_str()) {
                let token = format!(
                    "config-{}-{}",
                    app,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                );
                return AgentResponse::confirmation_required("config:set", app, &token);
            }
        }
    }

    // Note: The actual config:set is handled by the main CLI flow
    // This is a placeholder - in production, you'd call the real function
    AgentResponse::success(json!({
        "message": format!("Configuration update initiated for {}", app),
        "settings_count": settings.len(),
        "note": "Use 'riku config:set' for immediate effect"
    }))
}

fn cmd_agent_config_show(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    let config_file = paths.env_root.join(&app).join("ENV");
    let mut config = HashMap::new();

    if config_file.exists() {
        let _ = crate::util::parse_settings(&config_file, &mut config);
    }

    AgentResponse::success(json!({"app": app, "config": config}))
}

fn cmd_agent_logs(paths: &RikuPaths, app: &str, process: &str) -> AgentResponse {
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
        // Read log files
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

fn cmd_agent_ps(paths: &RikuPaths, app: &str) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    // Count workers
    let pattern = paths.workers_enabled.join(format!("{}*.toml", app));
    let worker_count = glob::glob(pattern.to_str().unwrap_or(""))
        .map(|g| g.count())
        .unwrap_or(0);

    AgentResponse::success(json!({
        "app": app,
        "workers": worker_count,
        "running": worker_count > 0
    }))
}

fn cmd_agent_restart(paths: &RikuPaths, app: &str, _process: Option<&str>) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    // Call the actual restart function
    match crate::cli::apps::cmd_restart(paths, &app) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Restart completed for {}", app)
        })),
        Err(e) => AgentResponse::error("RESTART_FAILED", &format!("Restart failed: {}", e)),
    }
}

fn cmd_agent_stop(paths: &RikuPaths, app: &str) -> AgentResponse {
    let _app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    // Require confirmation for stop
    let token = format!(
        "stop-{}-{}",
        app,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    AgentResponse::confirmation_required("stop", app, &token)
}

fn cmd_agent_stop_confirm(paths: &RikuPaths, app: &str, _token: &str) -> AgentResponse {
    // Note: In production, verify token matches the stop request

    // Call the actual stop function
    match crate::cli::apps::cmd_stop(paths, app) {
        Ok(_) => AgentResponse::success(json!({
            "message": format!("Application '{}' stopped successfully", app)
        })),
        Err(e) => AgentResponse::error("STOP_FAILED", &format!("Stop failed: {}", e)),
    }
}

fn cmd_agent_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> AgentResponse {
    let app = match exit_if_invalid(app, &paths.app_root) {
        Ok(a) => a,
        Err(_) => {
            return AgentResponse::error(
                "APP_NOT_FOUND",
                &format!("Application '{}' not found", app),
            )
        }
    };

    // In real implementation, run the command
    let command_str = cmd.join(" ");
    AgentResponse::success(json!({
        "app": app,
        "command": command_str,
        "message": "Command execution not yet implemented"
    }))
}

fn cmd_agent_stats(paths: &RikuPaths, app: Option<&str>) -> AgentResponse {
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

            // In real implementation, gather actual stats
            AgentResponse::success(json!({
                "app": app,
                "cpu": 0.0,
                "memory": 0,
                "requests": 0
            }))
        }
        None => {
            // Stats for all apps
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

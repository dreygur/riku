/// Agent schema and help output: intro and JSON schema.
use anyhow::Result;
use serde_json::json;

use super::auth::{get_agent_identity, get_agent_scope};
use super::types::AgentScope;
use crate::config::RikuPaths;

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
            AgentScope::Full => vec!["apps", "logs", "ps", "config:get", "config:show", "stats", "deploy", "destroy", "restart", "stop", "run", "config:set", "config:unset", "init", "update", "install-plugins", "supervisor", "plugin", "hook", "container", "git-hook", "scp", "ns-shim", "dump-state", "setup"],
        },
        "scope": match scope {
            AgentScope::Readonly => "readonly",
            AgentScope::Staging => "staging",
            AgentScope::Production => "production",
            AgentScope::Full => "full",
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

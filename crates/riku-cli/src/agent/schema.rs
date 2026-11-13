/// Agent schema and help output: intro and JSON schema.
use anyhow::Result;
use serde_json::json;

use super::auth::AgentSession;
use super::types::AgentScope;
use crate::config::RikuPaths;

/// Show agent introduction and permissions
pub fn cmd_agent_intro(_paths: &RikuPaths, session: &AgentSession) -> Result<()> {
    let AgentSession {
        id: agent_id,
        scope,
    } = session;

    let response = json!({
        "welcome": "Riku AI Agent Interface",
        "version": env!("CARGO_PKG_VERSION"),
        "agent_id": agent_id,
        "permissions": match scope {
            AgentScope::Readonly => vec!["apps", "logs", "ps", "config:get", "config:show", "stats"],
            AgentScope::Staging => vec!["apps", "logs", "ps", "config:get", "config:show", "stats", "deploy", "restart", "config:set", "config:unset"],
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

    match serde_json::to_string_pretty(&response) {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("failed to encode the agent intro: {e}")),
    }
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
                "description": "Set environment variables and redeploy",
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
                "requires_confirmation": "when a key name contains one of confirmation_markers",
                "permissions": ["staging", "production"],
                "confirmation_markers": ["SECRET", "PASS", "PWD", "TOKEN", "_KEY", "AUTH", "CREDENTIAL", "PRIVATE", "DATABASE_URL"]
            },
            {
                "name": "config:unset",
                "description": "Remove environment variables and redeploy",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "keys": {
                        "type": "array",
                        "required": true,
                        "description": "Keys to remove"
                    }
                },
                "requires_confirmation": "when a key name contains one of confirmation_markers",
                "permissions": ["staging", "production"],
                "confirmation_markers": ["SECRET", "PASS", "PWD", "TOKEN", "_KEY", "AUTH", "CREDENTIAL", "PRIVATE", "DATABASE_URL"]
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
                "description": "Restart an application's running processes from the current build, without rebuilding. Use deploy to rebuild",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "process": {
                        "type": "string",
                        "required": false,
                        "description": "Only this service of a container app, or this process kind (web, worker, ...) of a Procfile app"
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
                "description": "Run a command in the app directory with its live env, without a shell. Output is capped at 64 KiB per stream and the command is killed after 300 seconds",
                "parameters": {
                    "app": {
                        "type": "string",
                        "required": true,
                        "description": "Application name"
                    },
                    "command": {
                        "type": "array",
                        "required": true,
                        "description": "Program and its arguments; put -- before arguments that start with -"
                    }
                },
                "returns": "exit_code, stdout, stderr, truncated, timed_out; a nonzero exit or timeout is COMMAND_FAILED with the same data",
                "requires_confirmation": true,
                "permissions": ["production"]
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
            "confirm_token": "string - pass back with --confirm after a human approves"
        },
        "exit_status": "1 when the response has an error, 0 otherwise (a confirmation request exits 0)",
        "error_codes": {
            "APP_NOT_FOUND": "Application does not exist",
            "KEY_NOT_FOUND": "Configuration key is not set",
            "PERMISSION_DENIED": "Agent lacks required permission",
            "INVALID_COMMAND": "Unknown command",
            "INVALID_PARAMETERS": "Missing or invalid parameters",
            "INVALID_TOKEN": "Confirmation token is unknown, used, expired or for another action",
            "RATE_LIMIT_EXCEEDED": "Too many requests",
            "STATE_UNAVAILABLE": "Rate limit or audit state cannot be read or written",
            "COMMAND_FAILED": "agent run could not start, exited nonzero or timed out",
            "CONFIG_FAILED": "The ENV file could not be written; nothing changed",
            "DEPLOY_FAILED": "Deploy failed (after config:set or config:unset the config was saved)",
            "RESTART_FAILED": "Restart failed",
            "STOP_FAILED": "Stop failed",
            "DESTROY_FAILED": "Destroy failed"
        }
    });

    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

/// Agent command dispatcher: validates rate limit, permission, then routes to command implementations.
use anyhow::Result;

use crate::config::RikuPaths;

use super::auth::{check_rate_limit_now, log_agent_action, AgentSession};
use super::commands::{
    cmd_agent_apps, cmd_agent_config_get, cmd_agent_config_set, cmd_agent_config_show,
    cmd_agent_deploy, cmd_agent_destroy_confirm, cmd_agent_destroy_request, cmd_agent_logs,
    cmd_agent_ps, cmd_agent_restart, cmd_agent_run, cmd_agent_stats, cmd_agent_stop,
    cmd_agent_stop_confirm,
};
use super::state::agent_dir;
use super::types::AgentResponse;

fn print_response(response: &AgentResponse) -> Result<()> {
    match serde_json::to_string(&response.to_json()) {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("failed to encode the agent response: {e}")),
    }
}

/// Execute agent command
pub fn cmd_agent_execute(
    paths: &RikuPaths,
    session: &AgentSession,
    command: &str,
    args: &[String],
    confirm_token: Option<&str>,
) -> Result<()> {
    let AgentSession {
        id: agent_id,
        scope,
    } = session;

    let state = match agent_dir(&paths.riku_root) {
        Ok(dir) => dir,
        Err(e) => {
            let message = format!("Agent state is unavailable: {e}");
            return print_response(&AgentResponse::error("STATE_UNAVAILABLE", &message));
        }
    };

    match check_rate_limit_now(&state, agent_id, scope) {
        Ok(true) => {}
        Ok(false) => {
            return print_response(&AgentResponse::error(
                "RATE_LIMIT_EXCEEDED",
                "Too many requests. Try again in 30 seconds.",
            ));
        }
        Err(e) => {
            let message = format!("Agent state is unavailable: {e}");
            return print_response(&AgentResponse::error("STATE_UNAVAILABLE", &message));
        }
    }

    if !scope.allows(command) {
        log_agent_action(&state, agent_id, command, "", false);
        return print_response(&AgentResponse::error(
            "PERMISSION_DENIED",
            "Agent lacks required permission",
        ));
    }

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

    log_agent_action(
        &state,
        agent_id,
        command,
        args.first().map(|s| s.as_str()).unwrap_or(""),
        response.success,
    );

    print_response(&response)
}

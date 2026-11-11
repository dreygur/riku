/// Agent command dispatcher: validates rate limit, permission, then routes to command implementations.
use anyhow::Result;

use crate::config::RikuPaths;

use super::audit::{log_agent_action, open_before_mutation, AuditEntry};
use super::auth::{check_rate_limit_now, AgentSession};
use super::commands::{
    cmd_agent_apps, cmd_agent_config_get, cmd_agent_config_set, cmd_agent_config_show,
    cmd_agent_config_unset, cmd_agent_deploy, cmd_agent_destroy_confirm, cmd_agent_destroy_request,
    cmd_agent_logs, cmd_agent_ps, cmd_agent_restart, cmd_agent_run, cmd_agent_stats,
    cmd_agent_stop, cmd_agent_stop_confirm,
};
use super::quiet_stdout::with_stdout_on_stderr;
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

/// Runs the command and prints the JSON response.
///
/// A response carrying an error code also returns `Err`, so the process exits
/// nonzero and a caller can tell failure apart without parsing stdout. A
/// confirmation request is not an error: it exits zero with its token.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn cmd_agent_execute(
    paths: &RikuPaths,
    session: &AgentSession,
    command: &str,
    args: &[String],
    confirm_token: Option<&str>,
) -> Result<()> {
    let run = || run_agent_command(paths, session, command, args, confirm_token);
    let response = match with_stdout_on_stderr(run) {
        Ok(response) => response,
        Err(e) => return Err(e),
    };
    match (print_response(&response), response.error) {
        (Err(e), _) => Err(e),
        (Ok(()), Some(error)) => Err(anyhow::anyhow!("agent {command} failed: {}", error.code)),
        (Ok(()), None) => Ok(()),
    }
}

fn run_agent_command(
    paths: &RikuPaths,
    session: &AgentSession,
    command: &str,
    args: &[String],
    confirm_token: Option<&str>,
) -> AgentResponse {
    let AgentSession {
        id: agent_id,
        scope,
    } = session;

    let state = match agent_dir(&paths.riku_root) {
        Ok(dir) => dir,
        Err(e) => {
            let message = format!("Agent state is unavailable: {e}");
            return AgentResponse::error("STATE_UNAVAILABLE", &message);
        }
    };

    match check_rate_limit_now(&state, agent_id, scope) {
        Ok(true) => {}
        Ok(false) => {
            return AgentResponse::error(
                "RATE_LIMIT_EXCEEDED",
                "Too many requests. Try again in 30 seconds.",
            );
        }
        Err(e) => {
            let message = format!("Agent state is unavailable: {e}");
            return AgentResponse::error("STATE_UNAVAILABLE", &message);
        }
    }

    if !scope.allows(command) {
        let entry = AuditEntry {
            agent: agent_id,
            action: command,
            app: "",
            success: false,
        };
        log_agent_action(&state, None, &entry);
        return AgentResponse::error("PERMISSION_DENIED", "Agent lacks required permission");
    }

    let audit_log = match open_before_mutation(&state, command) {
        Ok(file) => file,
        Err(e) => {
            let message = format!("Agent audit log is unavailable: {e}");
            return AgentResponse::error("STATE_UNAVAILABLE", &message);
        }
    };

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
                    cmd_agent_destroy_confirm(paths, &state, app, token)
                } else {
                    cmd_agent_destroy_request(paths, &state, app)
                }
            }
        }
        "config:get" => {
            if args.len() < 2 {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name or key")
            } else {
                cmd_agent_config_get(paths, scope, &args[0], &args[1])
            }
        }
        "config:set" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_config_set(paths, &state, &args[0], &args[1..], confirm_token)
            }
        }
        "config:unset" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_config_unset(paths, &state, &args[0], &args[1..], confirm_token)
            }
        }
        "config:show" => {
            if args.is_empty() {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name")
            } else {
                cmd_agent_config_show(paths, scope, &args[0])
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
                    cmd_agent_stop_confirm(paths, &state, app, token)
                } else {
                    cmd_agent_stop(paths, &state, app)
                }
            }
        }
        "run" => {
            if args.len() < 2 {
                AgentResponse::error("INVALID_PARAMETERS", "Missing app name or command")
            } else {
                cmd_agent_run(paths, &state, &args[0], &args[1..], confirm_token)
            }
        }
        "stats" => {
            let app = args.first().map(|s| s.as_str());
            cmd_agent_stats(paths, app)
        }
        _ => AgentResponse::error("INVALID_COMMAND", &format!("Unknown command: {}", command)),
    };

    let entry = AuditEntry {
        agent: agent_id,
        action: command,
        app: args.first().map(|s| s.as_str()).unwrap_or(""),
        success: response.success,
    };
    log_agent_action(&state, audit_log, &entry);

    response
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;

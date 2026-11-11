//! Per-command help text for the agent CLI interface.

use anyhow::Result;

/// Show help for a specific command or the general agent interface.
pub fn cmd_agent_help(command: Option<&str>) -> Result<()> {
    match command {
        Some(cmd) => print_command_help(cmd),
        None => print_general_help(),
    }
    Ok(())
}

fn print_command_help(cmd: &str) {
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

Deploy an application and wait for the deploy to finish.
Requires 'staging' or 'production' scope.

Example:
  riku agent deploy myapp

Response:
  {"success": true, "data": {"status": "completed", "message": "..."}}"#
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
            r#"Usage: riku agent logs <app> [process]

View the last 100 lines of each log file of an application.

Example:
  riku agent logs myapp web

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
            r#"Usage: riku agent stop <app> [--confirm <token>]

Stop an application. Requires 'production' scope and human confirmation.

Example:
  # First request (gets confirmation token)
  riku agent stop myapp

  # After human confirms
  riku agent stop myapp --confirm abc123

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

fn print_general_help() {
    println!("AI Agent Interface for Riku");
    println!();
    println!("Usage: riku agent [OPTIONS] [COMMAND] [ARGS]...");
    println!();
    println!("Responses are one JSON object on stdout. A response with an");
    println!("error code exits 1.");
    println!();
    println!("Options:");
    println!("  --intro       Show agent introduction and permissions");
    println!("  --schema      Show full command schema (JSON)");
    println!("  --agent-help  Show this help, or help for the named command");
    println!("  --confirm     Confirmation token for destroy and stop");
    println!();
    println!("Commands:");
    println!("  apps        List deployed applications");
    println!("  deploy      Deploy an application");
    println!("  destroy     Remove an application");
    println!("  config:get  Get configuration value");
    println!("  config:set  Not implemented yet (use riku config set)");
    println!("  config:show Show all configuration");
    println!("  logs        View application logs");
    println!("  ps          Show process status");
    println!("  restart     Restart an application");
    println!("  stop        Stop an application");
    println!("  run         Not implemented yet (use riku run)");
    println!("  stats       Show statistics");
    println!();
    println!("Use 'riku agent --agent-help <command>' for command-specific help.");
}

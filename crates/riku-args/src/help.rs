//! Top-level `riku -h` output, grouped by where each command runs.
//!
//! clap's template language has no escape for `{`, so command descriptions are
//! sanitized before they are spliced into the template.

use clap::{Command, CommandFactory};

use crate::address::ServerName;
use crate::cli::Cli;
use crate::placement::{placement_of, Placement};

const LOCAL_HEADING: &str = "Local commands:";
const SERVER_HEADING: &str = "Server commands:";
const HELP_ABOUT: &str = "Print this message or the help of the given subcommand(s)";
const CONTAINER: &str = "container";
const DASHBOARD: &str = "dashboard";
const CLIENT_FORWARD_HEADING: &str = "Forwarded commands (run on the selected server):";

type Row = (String, String);

/// Forwarded group title when no server is implied by the repo or a default.
pub const FORWARD_HEADING: &str =
    "Forwarded commands (run on the selected server, or here with --local):";

fn escape_braces(text: &str) -> String {
    text.replace('{', "(").replace('}', ")")
}

/// Forwarded group title naming the server a command without `-s` reaches.
pub fn forward_heading(server: Option<&ServerName>) -> String {
    match server {
        Some(name) => format!(
            "Forwarded commands (run on {}, or here with --local):",
            name.as_str()
        ),
        None => FORWARD_HEADING.to_string(),
    }
}

fn visible_in(root: &Command, placement: Placement) -> Vec<(String, String)> {
    root.get_subcommands()
        .filter(|c| !c.is_hide_set() && placement_of(c.get_name()) == Some(placement))
        .map(|c| {
            let about = c.get_about().map(|s| s.to_string()).unwrap_or_default();
            let first_line = about.lines().next().unwrap_or("").to_string();
            (c.get_name().to_string(), escape_braces(&first_line))
        })
        .collect()
}

fn group_text(heading: &str, rows: &[(String, String)], width: usize) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let lines: String = rows
        .iter()
        .map(|(name, about)| format!("  {name:<width$}  {about}\n"))
        .collect();
    format!("{heading}\n{lines}\n")
}

/// Returns the root `Cli` command with a help template that lists commands in
/// Local, Server, Forwarded order. `forward_heading` titles the last group.
pub fn command(forward_heading: &str) -> Command {
    let root = Cli::command();
    let mut local = visible_in(&root, Placement::Local);
    local.push(("help".to_string(), HELP_ABOUT.to_string()));
    let server = visible_in(&root, Placement::Server);
    let forwarded = visible_in(&root, Placement::Forwarded);
    with_groups(root, forward_heading, [local, server, forwarded])
}

/// The root command as `rk` shows it: only what a laptop can run. The remote
/// dashboard counts as local there, `container` and the server only commands
/// are left out, and `--local` is hidden since `rk` has no local mode.
pub fn client_command(server: Option<&ServerName>) -> Command {
    let root = Cli::command().name("rk").mut_arg("local", |a| a.hide(true));
    let mut local: Vec<_> = visible_in(&root, Placement::Local)
        .into_iter()
        .filter(|(name, _)| name != CONTAINER)
        .collect();
    local.extend(
        visible_in(&root, Placement::Server)
            .into_iter()
            .filter(|(name, _)| name == DASHBOARD),
    );
    local.push(("help".to_string(), HELP_ABOUT.to_string()));
    let forwarded = visible_in(&root, Placement::Forwarded);
    let heading = match server {
        Some(name) => format!("Forwarded commands (run on {}):", name.as_str()),
        None => CLIENT_FORWARD_HEADING.to_string(),
    };
    with_groups(root, &heading, [local, Vec::new(), forwarded])
}

fn with_groups(root: Command, forward_heading: &str, rows: [Vec<Row>; 3]) -> Command {
    let [local, server, forwarded] = rows;
    let width = local
        .iter()
        .chain(&server)
        .chain(&forwarded)
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    let groups = [
        group_text(LOCAL_HEADING, &local, width),
        group_text(SERVER_HEADING, &server, width),
        group_text(&escape_braces(forward_heading), &forwarded, width),
    ]
    .concat();
    let template = format!(
        "{{about-with-newline}}\n{{usage-heading}} {{usage}}\n\n{groups}Options:\n{{options}}"
    );
    root.help_template(template)
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

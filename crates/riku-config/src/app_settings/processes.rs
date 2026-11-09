//! `[processes]` and `[cron]`: the Procfile replacement.
//!
//! Kinds follow the Procfile rules (`riku_util::procfile::is_valid_kind`) and
//! cron schedules get the Procfile's own check, so both sources agree on what
//! is valid. `[cron] backup = "0 2 * * * ./backup.sh"` becomes kind
//! `cron-backup`. Commands run through `sh -c` exactly as Procfile commands
//! do, so they are only refused control characters.

use std::collections::BTreeMap;

use riku_util::procfile::{is_valid_cron_command, is_valid_kind};

use super::Problem;

const CRON_PREFIX: &str = "cron";
const WSGI_KINDS: [&str; 3] = ["wsgi", "jwsgi", "rwsgi"];

fn command_at(section: &str, key: &str, value: &toml::Value) -> Result<String, Problem> {
    match value.as_str() {
        Some(text) if !text.trim().is_empty() && !text.chars().any(char::is_control) => {
            Ok(text.trim().to_string())
        }
        _ => Err(Problem::at(
            section,
            key,
            format!("{section}.{key} must be a non-empty one-line command"),
        )),
    }
}

/// The kind for `key` in `section`, or why it is refused.
fn kind_for(section: &str, key: &str) -> Result<String, Problem> {
    if !is_valid_kind(key) {
        return Err(Problem::at(
            section,
            key,
            format!("{section}.{key}: names may only hold letters, digits, _ and -"),
        ));
    }
    if section == "cron" {
        return Ok(format!("{CRON_PREFIX}-{key}"));
    }
    if key.starts_with(CRON_PREFIX) {
        return Err(Problem::at(
            section,
            key,
            format!("processes.{key}: put cron jobs under [cron]"),
        ));
    }
    Ok(key.to_string())
}

/// Add the entries of `[processes]` or `[cron]` to `out`.
pub fn add_section(
    section: &str,
    table: &toml::Table,
    out: &mut BTreeMap<String, String>,
) -> Result<(), Problem> {
    for (key, value) in table {
        let kind = match kind_for(section, key) {
            Ok(kind) => kind,
            Err(problem) => return Err(problem),
        };
        let command = match command_at(section, key, value) {
            Ok(command) => command,
            Err(problem) => return Err(problem),
        };
        if section == "cron" && !is_valid_cron_command(&command) {
            return Err(Problem::at(
                section,
                key,
                format!("cron.{key} must start with a 5 field cron schedule"),
            ));
        }
        out.insert(kind, command);
    }
    Ok(())
}

/// Refuse `web` next to a wsgi kind. The Procfile silently drops `web` in
/// that case; a new file should say what it means.
pub fn check_web_and_wsgi(processes: &BTreeMap<String, String>) -> Result<(), Problem> {
    let has_wsgi = WSGI_KINDS.iter().any(|kind| processes.contains_key(*kind));
    if has_wsgi && processes.contains_key("web") {
        return Err(Problem::at(
            "processes",
            "web",
            "processes: use either web or a wsgi kind, not both".to_string(),
        ));
    }
    Ok(())
}

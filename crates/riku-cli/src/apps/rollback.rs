//! `riku rollback` provider: roll an app back to a previous release.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::display;

/// `riku rollback <app> [--to <sha>] [--list]`
pub fn cmd_rollback(paths: &RikuPaths, app: &str, to: Option<&str>, list: bool) -> Result<()> {
    if list {
        return show_history(paths, app);
    }
    crate::deploy::rollback(app, paths, to)
}

fn show_history(paths: &RikuPaths, app: &str) -> Result<()> {
    let history = crate::deploy::releases::ReleaseLog::new(paths).list(app);
    if history.is_empty() {
        display::detail(&format!("No release history for '{app}' yet."));
        return Ok(());
    }
    // Newest first; the first row is the current release.
    let rows: Vec<Vec<String>> = history
        .iter()
        .rev()
        .enumerate()
        .map(|(i, r)| {
            vec![
                r.sha.chars().take(12).collect(),
                fmt_ts(r.ts),
                if i == 0 { "current" } else { "" }.to_string(),
            ]
        })
        .collect();
    display::print_table(&["RELEASE", "DEPLOYED", ""], &rows, 2);
    Ok(())
}

fn fmt_ts(ts: u64) -> String {
    match jiff::Timestamp::from_second(ts as i64) {
        Ok(at) => at.strftime("%Y-%m-%d %H:%M UTC").to_string(),
        Err(_) => ts.to_string(),
    }
}

//! `riku doctor` — diagnose the local Riku installation.
//!
//! Solo devs have no ops team, so the tool *is* the ops team: `doctor` runs a
//! battery of **read-only** health checks across dependencies, the `~/.riku`
//! directory layout, the systemd supervisor service, nginx, runtime plugins,
//! disk headroom, and SSH deploy access, then prints a summary. It never
//! mutates state, so it is always safe to run.
//!
//! Exit code: `0` when nothing failed (warnings allowed), `1` when any check
//! has [`Status::Fail`], so it composes in CI and provisioning scripts.

mod checks;

use anyhow::Result;
use colored::Colorize;

use crate::config::RikuPaths;
use crate::util::display;

/// Outcome of a single diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

/// A single diagnostic result: what was inspected, the outcome, and a
/// human-readable detail (a remediation hint on `Warn`/`Fail`).
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    fn new(name: impl Into<String>, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status,
            detail: detail.into(),
        }
    }

    pub fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Ok, detail)
    }

    pub fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Warn, detail)
    }

    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(name, Status::Fail, detail)
    }
}

/// Run every diagnostic, render the report, and exit non-zero on any failure.
pub fn cmd_doctor(paths: &RikuPaths) -> Result<()> {
    display::info("Running Riku diagnostics...");
    display::blank();

    let checks = collect(paths);
    render(&checks);
    summarize_and_exit(&checks);

    Ok(())
}

/// Gather all checks in display order.
fn collect(paths: &RikuPaths) -> Vec<Check> {
    let mut checks = Vec::new();
    checks.extend(checks::dependencies());
    checks.push(checks::directories(paths));
    checks.extend(checks::binary());
    checks.push(checks::systemd_service());
    checks.extend(checks::nginx());
    checks.push(checks::plugins(paths));
    checks.push(checks::disk(paths));
    checks.push(checks::ssh_access());
    checks
}

/// Print one line per check (plus an indented detail line).
fn render(checks: &[Check]) {
    for c in checks {
        let line = match c.status {
            Status::Ok => format!("{} {}", "✓".green().bold(), c.name.bold()),
            Status::Warn => format!("{} {}", "!".yellow().bold(), c.name.yellow().bold()),
            Status::Fail => format!("{} {}", "✗".red().bold(), c.name.red().bold()),
        };
        println!("  {line}");
        if !c.detail.is_empty() {
            println!("      {}", c.detail.dimmed());
        }
    }
}

/// Count checks by status: `(ok, warn, fail)`.
fn tally(checks: &[Check]) -> (usize, usize, usize) {
    let mut ok = 0;
    let mut warn = 0;
    let mut fail = 0;
    for c in checks {
        match c.status {
            Status::Ok => ok += 1,
            Status::Warn => warn += 1,
            Status::Fail => fail += 1,
        }
    }
    (ok, warn, fail)
}

/// Print the tally and terminate the process with an appropriate exit code.
fn summarize_and_exit(checks: &[Check]) {
    let (ok, warn, fail) = tally(checks);

    display::blank();
    let summary = format!("{ok} ok, {warn} warning(s), {fail} failure(s)");
    if fail > 0 {
        display::error(&format!("Diagnostics found problems: {summary}"));
        std::process::exit(1);
    } else if warn > 0 {
        display::warn(&format!("Diagnostics passed with warnings: {summary}"));
    } else {
        display::success(&format!("All checks passed: {summary}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RikuPaths;

    #[test]
    fn tally_counts_each_status() {
        let checks = vec![
            Check::ok("a", ""),
            Check::ok("b", ""),
            Check::warn("c", ""),
            Check::fail("d", ""),
        ];
        assert_eq!(tally(&checks), (2, 1, 1));
    }

    #[test]
    fn directories_fails_when_tree_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // Root exists but the sub-directories do not.
        let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
        let check = checks::directories(&paths);
        assert_eq!(check.status, Status::Fail);
        assert!(check.detail.contains("apps"));
    }

    #[test]
    fn directories_ok_when_tree_present() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
        for dir in [
            &paths.app_root,
            &paths.data_root,
            &paths.env_root,
            &paths.git_root,
            &paths.log_root,
            &paths.nginx_root,
            &paths.plugin_root,
            &paths.workers_enabled,
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let check = checks::directories(&paths);
        assert_eq!(check.status, Status::Ok);
    }

    #[test]
    fn plugins_warns_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
        std::fs::create_dir_all(&paths.plugin_root).unwrap();
        assert_eq!(checks::plugins(&paths).status, Status::Warn);
    }

    #[test]
    fn plugins_ok_when_populated() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
        std::fs::create_dir_all(&paths.plugin_root).unwrap();
        std::fs::write(paths.plugin_root.join("node"), "#!/bin/sh\n").unwrap();
        let check = checks::plugins(&paths);
        assert_eq!(check.status, Status::Ok);
        assert!(check.detail.contains('1'));
    }

    #[test]
    fn disk_reports_free_space_for_existing_root() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().to_path_buf(), tmp.path());
        // A tempdir on a normal dev/CI box has well over 100 MiB free.
        assert_ne!(checks::disk(&paths).status, Status::Fail);
    }

    #[test]
    fn check_constructors_set_status() {
        assert_eq!(Check::ok("n", "d").status, Status::Ok);
        assert_eq!(Check::warn("n", "d").status, Status::Warn);
        assert_eq!(Check::fail("n", "d").status, Status::Fail);
        // Detail is preserved verbatim.
        assert_eq!(Check::ok("n", "hello").detail, "hello".to_string());
    }
}

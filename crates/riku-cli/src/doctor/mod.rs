//! `riku doctor`: diagnose the local Riku installation.
//!
//! Solo devs have no ops team, so the tool *is* the ops team: `doctor` runs a
//! battery of **read-only** health checks across dependencies, the `~/.riku`
//! directory layout, the systemd supervisor service, the host router, the
//! container engine, runtime plugins, disk headroom, SSH deploy access, and
//! renamed app ENV keys, then prints a summary.
//!
//! Exit code: `0` when nothing failed (warnings allowed), `1` when any check
//! has [`Status::Fail`], so it composes in CI and provisioning scripts.

pub mod app_env;
pub mod binary_check;
pub mod checks;
pub mod container_check;
pub mod helper_check;
mod migrate_check;
mod migrate_env;
pub mod migrate_settings;
mod mode_write;
pub mod own_cert_check;
pub mod package_tools;
pub mod redeploy_sites;
pub mod router_check;
pub mod settings_check;
pub mod stale_paths;
pub mod supervisor_check;

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::config::router::Router;
use crate::config::{RikuPaths, CADDY_SITE_ROOT, NGINX_SITE_ROOT};
use crate::setup::deploy_user::{running_as_root, DeployUser};
use crate::util::display::{self, Mark};

/// Outcome of a single diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

/// A single diagnostic result: what was inspected, the outcome, and a
/// human-readable detail (a remediation hint on `Warn`/`Fail`).
#[derive(Debug)]
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

/// The riku tree to diagnose. Under `sudo`, HOME is root's, so a root run
/// checks the deploy user's tree instead, the one deploys and the supervisor
/// use.
fn diagnosed_target(paths: &RikuPaths) -> Result<RikuPaths> {
    if !running_as_root() {
        return Ok(paths.clone());
    }
    match DeployUser::lookup() {
        Ok(user) => Ok(user.riku_paths()),
        Err(e) => Err(e),
    }
}

/// Redeploy the apps that have no site file in the active router's site
/// root. A deploy runs the app's build, which is the deploy user's code, so it
/// is refused as root. A plugin router keeps its own sites, so nothing is
/// redeployed for it.
fn redeploy_old_sites(paths: &RikuPaths) -> Vec<Check> {
    if nix::unistd::Uid::effective().is_root() {
        return vec![Check::warn(
            "router sites",
            "redeploy skipped as root: run `riku doctor --fix` as the deploy user",
        )];
    }
    let (site_root, extension) = match Router::load(paths) {
        Ok(Router::Nginx) => (NGINX_SITE_ROOT, "conf"),
        Ok(Router::Caddy) => (CADDY_SITE_ROOT, "caddy"),
        Ok(Router::Plugin(_)) => return Vec::new(),
        Err(e) => return vec![Check::fail("router sites", e.to_string())],
    };
    redeploy_sites::redeploy_unrouted_sites(
        &paths.riku_root,
        Path::new(site_root),
        extension,
        // An app whose routing cannot be read is redeployed, as before;
        // the deploy then reports why.
        |app| crate::deploy::router::site_expected(app, paths).unwrap_or(true),
        |app| crate::deploy::do_deploy(app, paths, &HashMap::new(), None),
    )
}

/// Rewrite stale forced commands and post-receive hooks. They are the deploy
/// user's own files, so this is refused as root, like the redeploy step.
fn fix_launch_paths(paths: &RikuPaths) -> Vec<Check> {
    if nix::unistd::Uid::effective().is_root() {
        return vec![Check::warn(
            "launch paths",
            "rewrite skipped as root: run `riku doctor --fix` as the deploy user",
        )];
    }
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(_) => {
            return vec![Check::warn(
                "launch paths",
                "HOME not set, nothing rewritten",
            )]
        }
    };
    let keys = crate::util::authorized_keys::authorized_keys_path(Path::new(&home));
    stale_paths::fix_stale_launch_paths(&keys, &paths.git_root)
}

/// Run every diagnostic, render the report, and exit non-zero on any failure.
/// With `fix`, apps missing from the router's site root are redeployed
/// and stale launch paths rewritten first, so the checks see the result.
#[allow(clippy::question_mark)]
pub fn cmd_doctor(paths: &RikuPaths, fix: bool) -> Result<()> {
    let paths = match diagnosed_target(paths) {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    display::intro(&format!("riku doctor  {}", paths.riku_root.display()));

    let mut checks = Vec::new();
    if fix {
        checks.extend(redeploy_old_sites(&paths));
        checks.extend(migrate_settings::migrate_settings(&paths));
        checks.extend(fix_launch_paths(&paths));
    }
    checks.extend(collect(&paths));
    render(&checks);
    summarize_and_exit(&checks);

    Ok(())
}

/// Gather all checks in display order. The dashboard's `/api/doctor` serves
/// this same list.
pub fn collect(paths: &RikuPaths) -> Vec<Check> {
    let mut checks = Vec::new();
    checks.extend(checks::dependencies());
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    checks.extend(package_tools::package_tools(home.as_deref()));
    checks.push(checks::directories(paths));
    checks.extend(binary_check::binary());
    checks.push(checks::systemd_service());
    let deploy_user = crate::setup::deploy_user::deploy_user_name();
    checks.extend(supervisor_check::supervisor_binary(
        supervisor_check::Runner::current(&deploy_user),
        &paths.riku_root.join("supervisor.pid"),
        Path::new("/proc"),
    ));
    checks.push(helper_check::router_helper(paths));
    checks.extend(router_check::router(paths));
    checks.push(settings_check::settings_file(paths));
    checks.extend(settings_check::legacy_files(paths));
    checks.extend(settings_check::routing_section(paths));
    checks.push(container_check::container_engine(paths));
    checks.push(checks::plugins(paths));
    checks.push(checks::disk(paths));
    checks.push(checks::ssh_access());
    checks.extend(app_env::renamed_keys(paths));
    checks.extend(own_cert_check::own_certificates(paths));
    checks
}

/// The rail line of one check, its detail on the `│` lines under it. All on
/// stdout, so a piped report keeps its order.
fn render_check(check: &Check, colour: bool) -> String {
    let mark = match check.status {
        Status::Ok => Mark::Step,
        Status::Warn => Mark::Warn,
        Status::Fail => Mark::Error,
    };
    let mut out = display::render_line(mark, &check.name, "", colour);
    for line in check.detail.lines() {
        out.push('\n');
        out.push_str(&display::render_line(Mark::Rail, line, "", colour));
    }
    out
}

fn render(checks: &[Check]) {
    let colour = display::Stream::Stdout.colour();
    for check in checks {
        println!("{}", render_check(check, colour));
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
    let counts = format!("{ok} ok, {warn} warning(s), {fail} failure(s)");
    let outro = match (warn, fail) {
        (_, 1..) => "Diagnostics found problems",
        (1.., 0) => "Diagnostics passed with warnings",
        _ => "All checks passed",
    };
    display::outro(outro);
    display::summary(&[("Checks", &counts)]);
    if fail > 0 {
        std::process::exit(1);
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
        std::fs::write(paths.plugin_root.join(".bundled"), "").unwrap();
        assert_eq!(
            checks::plugins(&paths).status,
            Status::Warn,
            "the hidden bundled record counted as a plugin"
        );
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

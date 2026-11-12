//! `riku doctor`: diagnose the local Riku installation.
//!
//! Solo devs have no ops team, so the tool *is* the ops team: `doctor` runs a
//! battery of **read-only** health checks across dependencies, the `~/.riku`
//! directory layout, the systemd supervisor service, the host router, the
//! container engine, runtime plugins, disk headroom, SSH deploy access and
//! own certificates, then prints a summary.
//!
//! Exit code: `0` when nothing failed (warnings allowed), `1` when any check
//! has [`Status::Fail`], so it composes in CI and provisioning scripts.

pub mod binary_check;
pub mod checks;
pub mod container_check;
pub mod helper_check;
pub mod own_cert_check;
pub mod package_tools;
mod report;
pub mod router_check;
pub mod router_dirs_check;
pub mod settings_check;
pub mod ssh_check;
pub mod supervisor_check;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::{RikuPaths, CERT_ROOT, NGINX_SITE_ROOT};
use crate::setup::deploy_user::{running_as_root, DeployUser};
use crate::util::display;
use crate::util::write_dir::ROOT_UID;

use report::{render, summarize_and_exit};

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

/// The home of the user whose tree is diagnosed: the deploy user's under
/// root, as in [`diagnosed_target`], and HOME otherwise.
fn diagnosed_home() -> Option<PathBuf> {
    if !running_as_root() {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    DeployUser::lookup().ok().map(|user| user.home)
}

/// Run every diagnostic, render the report, and exit non-zero on any failure.
#[allow(clippy::question_mark)]
pub fn cmd_doctor(paths: &RikuPaths) -> Result<()> {
    let paths = match diagnosed_target(paths) {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    display::intro(&format!("riku doctor  {}", paths.riku_root.display()));

    let checks = collect(&paths);
    render(&checks);
    summarize_and_exit(&checks);

    Ok(())
}

/// Gather all checks in display order. The dashboard's `/api/doctor` serves
/// this same list.
pub fn collect(paths: &RikuPaths) -> Vec<Check> {
    let mut checks = Vec::new();
    checks.extend(checks::dependencies());
    let home = diagnosed_home();
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
    checks.extend(router_dirs_check::router_dirs(
        &[
            PathBuf::from(NGINX_SITE_ROOT),
            PathBuf::from(CERT_ROOT),
            paths.caddy_root.clone(),
            paths.caddy_root.join("certs"),
        ],
        ROOT_UID,
    ));
    checks.push(settings_check::settings_file(paths));
    checks.extend(settings_check::routing(paths));
    checks.extend(container_check::container_engine(paths));
    checks.push(checks::plugins(paths));
    checks.push(checks::disk(paths));
    checks.push(match &home {
        Some(home) => ssh_check::ssh_access(home, ssh_check::Reader::current()),
        None => Check::warn(
            "ssh access",
            "home not found, cannot locate authorized_keys",
        ),
    });
    checks.extend(own_cert_check::own_certificates(paths));
    checks
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

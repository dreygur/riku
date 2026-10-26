//! `sudo riku init`: everything that belongs to the deploy user is done as
//! the deploy user, then root does only what needs root.
//!
//! Root copies the running binary into a fresh root-owned temp directory
//! and runs `riku init --user-steps` from there as the deploy user. That
//! child installs itself into `~/.local/bin`, creates `~/.riku`, writes the
//! `ROUTER` file and installs the git hook, all in its own home and owned
//! by it. Root then reads the router back, prepares the router on the host
//! and installs the systemd units.
//!
//! Security: root never writes in the deploy user's home and never runs a
//! binary from it: the deploy user could swap one between install and exec.
//! The staged copy sits in a directory only root can write. The child gets
//! no terminal (see [`DeployUser`]), so SSH key selection, which prompts,
//! is left to the deploy user's own `riku init`.

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{anyhow, Result};
use tempfile::TempDir;

use crate::util::echo;

use super::deploy_user::DeployUser;
use super::init_router::{deploy_router, setup_router_host};
use super::prerequisites::check_prerequisites;
use super::systemd::install_systemd_service;

const BINARY_NAME: &str = "riku";
const STAGE_PREFIX: &str = "riku-init-";
const STAGE_MODE: u32 = 0o755;

/// A root-owned copy of the running binary the deploy user can execute but
/// not change. The directory is removed when the returned guard drops.
#[allow(clippy::question_mark)]
fn stage_binary() -> Result<TempDir> {
    let source = match env::current_exe() {
        Ok(path) => path,
        Err(e) => return Err(anyhow!("failed to locate the riku binary: {e}")),
    };
    let stage = match tempfile::Builder::new().prefix(STAGE_PREFIX).tempdir() {
        Ok(dir) => dir,
        Err(e) => return Err(anyhow!("failed to create a staging directory: {e}")),
    };
    let staged = stage.path().join(BINARY_NAME);
    if let Err(e) = fs::copy(&source, &staged) {
        return Err(anyhow!("failed to stage {}: {e}", source.display()));
    }
    for path in [stage.path(), staged.as_path()] {
        if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(STAGE_MODE)) {
            return Err(anyhow!("failed to chmod {}: {e}", path.display()));
        }
    }
    Ok(stage)
}

fn print_ssh_hint(user: &DeployUser) {
    echo("[3/4] SSH key setup...", "");
    echo(
        &format!(
            "      ℹ Add a deploy key as {name}: sudo -iu {name} riku init --no-systemd",
            name = user.name
        ),
        "yellow",
    );
    echo("", "");
}

#[allow(clippy::question_mark)]
fn run_user_steps(user: &DeployUser) -> Result<()> {
    let stage = match stage_binary() {
        Ok(stage) => stage,
        Err(e) => return Err(e),
    };
    let binary = stage.path().join(BINARY_NAME);
    user.run_relayed(&binary, &["init", "--user-steps"])
}

/// Run a root init: per-user steps as the deploy user, then host setup.
#[allow(clippy::question_mark)]
pub fn root_init(no_systemd: bool) -> Result<()> {
    let user = match DeployUser::lookup() {
        Ok(user) => user,
        Err(e) => return Err(e),
    };
    echo(
        &format!(
            "-----> Initializing Riku for deploy user '{}'...",
            user.name
        ),
        "",
    );
    echo("", "");

    if let Err(e) = run_user_steps(&user) {
        return Err(e);
    }
    let router = match deploy_router(&user) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    check_prerequisites(true, &router);
    if let Err(e) = setup_router_host(&router, &user) {
        return Err(e);
    }

    if no_systemd {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
    } else {
        echo("[2/4] Setting up systemd service...", "");
        if let Err(e) = install_systemd_service(&router) {
            return Err(e);
        }
    }
    echo("", "");
    print_ssh_hint(&user);
    Ok(())
}

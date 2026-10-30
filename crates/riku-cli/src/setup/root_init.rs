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
//! no terminal (see [`DeployUser`]), so it cannot prompt for an SSH key:
//! keys come from `--ssh-key`, read here as root and passed on validated.

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{anyhow, Result};
use tempfile::TempDir;

use crate::util::echo;
use crate::util::public_key::PublicKey;

use super::deploy_account::ensure_account;
use super::deploy_keys::read_key_file;
use super::deploy_user::{deploy_user_name, DeployUser};
use super::init::InitOptions;
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

fn print_ssh_hint() {
    echo("[3/4] SSH key setup...", "");
    echo(
        "      ℹ No deploy key added. git push needs one, add yours with:",
        "yellow",
    );
    echo(
        "        sudo riku init --ssh-key <your .pub or authorized_keys file>",
        "yellow",
    );
    echo("", "");
}

#[allow(clippy::question_mark)]
fn run_user_steps(user: &DeployUser, keys: &[PublicKey]) -> Result<()> {
    let stage = match stage_binary() {
        Ok(stage) => stage,
        Err(e) => return Err(e),
    };
    let binary = stage.path().join(BINARY_NAME);
    let key_args: Vec<String> = keys
        .iter()
        .flat_map(|key| ["--add-key".to_string(), key.to_string()])
        .collect();
    let mut args = vec!["init", "--user-steps"];
    args.extend(key_args.iter().map(String::as_str));
    user.run_relayed(&binary, &args)
}

/// Run a root init: per-user steps as the deploy user, then host setup.
/// Returns the deploy user it set up.
#[allow(clippy::question_mark)]
pub fn root_init(options: &InitOptions) -> Result<DeployUser> {
    // Read as root, before any change, so a wrong file fails the init early.
    let keys = match &options.ssh_key {
        Some(file) => read_key_file(file),
        None => Ok(Vec::new()),
    };
    let keys = match keys {
        Ok(keys) => keys,
        Err(e) => return Err(e),
    };

    if let Err(e) = ensure_account(&deploy_user_name()) {
        return Err(e);
    }
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

    if let Err(e) = run_user_steps(&user, &keys) {
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

    if options.no_systemd {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
    } else {
        echo("[2/4] Setting up systemd service...", "");
        if let Err(e) = install_systemd_service(&router) {
            return Err(e);
        }
    }
    echo("", "");
    if keys.is_empty() {
        print_ssh_hint();
    }
    Ok(user)
}

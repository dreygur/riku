//! `sudo riku init`: everything that belongs to the deploy user is done as
//! the deploy user, then root does only what needs root.
//!
//! Root first installs the running binary as `/usr/local/bin/riku` (see
//! [`super::binary`]) and runs `riku init --user-steps` from there as the
//! deploy user. That child creates `~/.riku`, writes the `ROUTER` file and
//! installs the git hook, all in its own home and owned by it. Root then
//! reads the router back, creates the uWSGI socket directory (see
//! [`super::socket_group`]) and the ACME and static file directories (see
//! [`super::var_root`]), prepares the router on the host, installs the router
//! helper (see [`super::helper_install`]) and the systemd units.
//!
//! Security: root never writes in the deploy user's home and never runs a
//! binary from it: the deploy user could swap one between install and exec.
//! The binary it runs is root owned in a root owned directory, so the
//! deploy user cannot change it. The child gets no terminal (see
//! [`DeployUser`]), so it cannot prompt for an SSH key: keys come from
//! `--ssh-key`, read here as root and passed on validated.

use std::path::Path;

use anyhow::Result;

use crate::config::router::Router;
use crate::util::echo;
use crate::util::public_key::PublicKey;
use crate::util::ROOT_BINARY;

use super::binary::install_root_binary;
use super::deploy_account::ensure_account;
use super::deploy_keys::read_key_file;
use super::deploy_user::{deploy_user_name, DeployUser};
use super::helper_install::install_router_helper;
use super::init::InitOptions;
use super::init_router::{deploy_router, setup_router_host};
use super::prerequisites::check_prerequisites;
use super::socket_group::setup_socket_root;
use super::systemd::install_systemd_service;
use super::var_root::setup_var_root;

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

fn run_user_steps(user: &DeployUser, keys: &[PublicKey]) -> Result<()> {
    let key_args: Vec<String> = keys
        .iter()
        .flat_map(|key| ["--add-key".to_string(), key.to_string()])
        .collect();
    let mut args = vec!["init", "--user-steps"];
    args.extend(key_args.iter().map(String::as_str));
    user.run_relayed(Path::new(ROOT_BINARY), &args)
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
    if let Err(e) = install_root_binary() {
        return Err(e);
    }

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
    // Before the router: nginx host setup adds its worker to the group.
    if let Err(e) = setup_socket_root(&user) {
        return Err(e);
    }
    if let Err(e) = setup_var_root(&user) {
        return Err(e);
    }
    if let Err(e) = setup_router_host(&router, &user) {
        return Err(e);
    }

    if options.no_systemd {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
    } else {
        if !matches!(router, Router::Plugin(_)) {
            if let Err(e) = install_router_helper(&user.name) {
                return Err(e);
            }
        }
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

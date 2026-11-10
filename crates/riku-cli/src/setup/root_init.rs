//! `sudo riku init`: everything that belongs to the deploy user is done as
//! the deploy user, then root does only what needs root.
//!
//! Root first chooses the router (see [`super::router_choice`]): on a fresh
//! install before changing anything, otherwise from what the deploy user
//! stored. It then installs the running binary as `/usr/local/bin/riku` (see
//! [`super::binary`]) and runs `riku init --user-steps` from there as the
//! deploy user, passing the router as `RIKU_ROUTER`. That child creates
//! `~/.riku`, stores the router in settings.toml and installs the git hook,
//! all in its own home and owned by it. Root then creates the uWSGI socket
//! directory (see
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
use super::deploy_user::{deploy_user_name, passwd_home, DeployUser};
use super::helper_install::install_router_helper;
use super::init::InitOptions;
use super::init_router::{deploy_router, setup_router_host};
use super::prerequisites::check_prerequisites;
use super::router_choice;
use super::socket_group::setup_socket_root;
use super::systemd::install_systemd_service;
use super::var_root::setup_var_root;

const RIKU_DIR: &str = ".riku";
const ROUTER_ENV: &str = "RIKU_ROUTER";

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

/// Whether this is a fresh install: no deploy account yet, or one without a
/// riku tree. Read before init creates either. `symlink_metadata` does not
/// follow links, so the check is safe to make as root.
fn deploy_tree_missing() -> bool {
    match passwd_home(&deploy_user_name()) {
        None => true,
        Some(home) => matches!(
            std::fs::symlink_metadata(Path::new(&home).join(RIKU_DIR)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        ),
    }
}

/// The router for an existing install: what the deploy user has stored,
/// read as that user, else nginx, unless `RIKU_ROUTER` asks for another.
#[allow(clippy::question_mark)]
fn existing_router(user: &DeployUser) -> Result<Router> {
    let stored = match deploy_router(user) {
        Ok(stored) => stored,
        Err(e) => return Err(e),
    };
    router_choice::resolve(router_choice::choose_router(
        &router_choice::installed_routers(),
        router_choice::requested_router(),
        Some(stored),
        false,
        router_choice::interactive(),
    ))
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
    // A fresh install chooses its router before anything changes, so a host
    // without a router fails here and not halfway through.
    let fresh_router = if deploy_tree_missing() {
        let choice = router_choice::choose_router(
            &router_choice::installed_routers(),
            router_choice::requested_router(),
            None,
            true,
            router_choice::interactive(),
        );
        match router_choice::resolve(choice) {
            Ok(router) => Some(router),
            Err(e) => return Err(e),
        }
    } else {
        None
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
    let router = match fresh_router {
        Some(router) => router,
        None => match existing_router(&user) {
            Ok(router) => router,
            Err(e) => return Err(e),
        },
    };
    // The user steps run as the deploy user and store this choice; the
    // variable reaches them through `DeployUser`'s passed environment.
    std::env::set_var(ROUTER_ENV, router.name());
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
    check_prerequisites();
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

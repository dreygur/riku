/// Server initialization: directory structure, git hook, systemd, SSH, and verification.
use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::echo;

use super::binary::require_root_binary;
use super::default_branch::{set_default_branch, BRANCH_ENV};
use super::deploy_keys::{add_deploy_keys, parse_key_args, read_key_file};
use super::deploy_user::running_as_root;
use super::git_hook::create_git_hook;
use super::guidance::{print_finish, InstallScope};
use super::init_router::persist_router;
use super::prerequisites::{check_prerequisites, warn_not_root};
use super::root_init::root_init;
use super::router_choice;
use super::ssh::setup_ssh_key_interactive;
use super::stale_binary::remove_stale_binary;
use super::systemd::setup_systemd_service;

#[allow(clippy::question_mark)]
fn create_directory_structure(paths: &RikuPaths, router: &Router) -> Result<()> {
    let mut dirs: Vec<(&str, &PathBuf)> = vec![
        ("apps", &paths.app_root),
        ("cache", &paths.cache_root),
        ("data", &paths.data_root),
        ("repos", &paths.git_root),
        ("envs", &paths.env_root),
        ("workers", &paths.workers_root),
        ("workers-available", &paths.workers_available),
        ("workers-enabled", &paths.workers_enabled),
        ("logs", &paths.log_root),
        ("acme", &paths.acme_cert_links),
        ("plugins", &paths.plugin_root),
    ];
    // Caddy's site directory sits outside the home; root init creates it.
    if *router != Router::Caddy {
        dirs.push(("nginx", &paths.nginx_root));
    }

    for (name, dir) in &dirs {
        if dir.exists() {
            echo(&format!("      ✓ ~/.riku/{} (exists)", name), "green");
            continue;
        }
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("failed to create {}: {e}", dir.display()));
        }
        echo(&format!("      ✓ ~/.riku/{}", name), "green");
    }

    Ok(())
}

#[allow(clippy::question_mark)]
fn default_branch_step() -> Result<()> {
    let home = match env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => return Err(anyhow!("HOME is not set")),
    };
    set_default_branch(&home, env::var(BRANCH_ENV).ok())
}

fn remove_old_user_binary() -> Result<()> {
    match env::var("HOME") {
        Ok(home) => remove_stale_binary(&PathBuf::from(home)),
        Err(_) => Err(anyhow!("HOME is not set")),
    }
}

/// Directories, git hook and default branch in `paths`' tree.
#[allow(clippy::question_mark)]
fn prepare_tree(paths: &RikuPaths, router: &Router) -> Result<()> {
    echo("[1/4] Creating directory structure...", "");
    if let Err(e) = create_directory_structure(paths, router) {
        return Err(e);
    }

    echo("", "");
    if let Err(e) = create_git_hook(paths) {
        return Err(e);
    }
    if let Err(e) = default_branch_step() {
        return Err(e);
    }
    echo("", "");
    Ok(())
}

fn setup_user_service(paths: &RikuPaths, router: &Router, no_systemd: bool) -> Result<()> {
    if no_systemd {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
        echo("", "");
        return Ok(());
    }
    echo("[2/4] Setting up systemd service...", "");
    let installed = setup_systemd_service(paths, router);
    echo("", "");
    installed
}

/// Only root can create the uWSGI socket directory, so a plain init says
/// when it is missing instead of letting the first wsgi deploy fail.
fn warn_no_socket_root(paths: &RikuPaths) {
    if paths.socket_root.is_dir() {
        return;
    }
    echo(
        &format!(
            "      warning: {} does not exist, so wsgi, jwsgi, rwsgi and php apps cannot \
             deploy. Create it with: sudo riku init",
            paths.socket_root.display()
        ),
        "yellow",
    );
    echo("", "");
}

/// What `riku init` was asked to do.
pub struct InitOptions {
    pub no_systemd: bool,
    /// Set by a root init when it runs the per-user steps as the deploy user.
    pub user_steps: bool,
    /// `--ssh-key`: a `.pub` or `authorized_keys` file whose keys to authorize.
    pub ssh_key: Option<PathBuf>,
    /// Keys a root init already read and validated, for the per-user run.
    pub add_keys: Vec<String>,
}

/// Authorize the keys given on the command line, or else prompt for one.
/// A root init gives the per-user run no terminal, so that run never
/// prompts; root prints how to add a key instead.
#[allow(clippy::question_mark)]
fn ssh_key_step(options: &InitOptions) -> Result<()> {
    let keys = match &options.ssh_key {
        Some(file) => read_key_file(file),
        None => parse_key_args(&options.add_keys),
    };
    let keys = match keys {
        Ok(keys) => keys,
        Err(e) => return Err(e),
    };
    if keys.is_empty() && options.user_steps {
        return Ok(());
    }

    echo("[3/4] SSH key setup...", "");
    let added = if keys.is_empty() {
        setup_ssh_key_interactive()
    } else {
        add_deploy_keys(&keys)
    };
    echo("", "");
    added
}

/// The router this init sets up. In the user steps of a root init, root has
/// already chosen and passed it as `RIKU_ROUTER`. A plain init chooses
/// itself; `fresh` is read before anything is created.
#[allow(clippy::question_mark)]
fn init_router_choice(paths: &RikuPaths, user_steps: bool) -> Result<Router> {
    let requested = router_choice::requested_router();
    if user_steps {
        return match requested {
            Some(router) => Ok(router),
            None => Router::load(paths),
        };
    }
    let fresh = matches!(
        std::fs::symlink_metadata(&paths.riku_root),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    );
    let stored = match crate::config::router::stored(paths) {
        Ok(stored) => stored,
        Err(e) => return Err(e),
    };
    router_choice::resolve(router_choice::choose_router(
        &router_choice::installed_routers(),
        requested,
        stored,
        fresh,
        router_choice::interactive(),
    ))
}

/// Init for the user running it, in that user's own tree. With
/// `user_steps`, a root init is driving: the warning, prerequisites and
/// systemd step are root's to do, and root prints the summary.
#[allow(clippy::question_mark)]
fn user_init(paths: &RikuPaths, options: &InitOptions) -> Result<()> {
    let user_steps = options.user_steps;
    if !user_steps {
        if let Err(e) = require_root_binary() {
            return Err(e);
        }
    }
    let router = match init_router_choice(paths, user_steps) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if let Err(e) = persist_router(paths, &router) {
        return Err(e);
    }
    if !user_steps {
        warn_not_root(&router);
        check_prerequisites();
        echo("-----> Initializing Riku server...", "");
        echo("", "");
    }

    if let Err(e) = prepare_tree(paths, &router) {
        return Err(e);
    }
    if user_steps {
        if let Err(e) = remove_old_user_binary() {
            return Err(e);
        }
    } else {
        if let Err(e) = setup_user_service(paths, &router, options.no_systemd) {
            return Err(e);
        }
        warn_no_socket_root(paths);
    }

    ssh_key_step(options)
}

fn own_scope() -> InstallScope {
    InstallScope::User {
        user: env::var("USER").unwrap_or_default(),
    }
}

/// Initialize Riku on a server.
///
/// As root, the per-user steps run as the deploy user (see
/// [`super::root_init`]) and root adds the host setup. Otherwise everything
/// happens in the caller's own tree.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn cmd_init(options: InitOptions) -> Result<()> {
    if running_as_root() {
        let user = match root_init(&options) {
            Ok(user) => user,
            Err(e) => return Err(e),
        };
        let scope = InstallScope::System { user: user.name };
        print_finish(&scope, options.no_systemd);
        return Ok(());
    }

    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    if let Err(e) = user_init(&paths, &options) {
        return Err(e);
    }
    if !options.user_steps {
        print_finish(&own_scope(), options.no_systemd);
    }
    Ok(())
}

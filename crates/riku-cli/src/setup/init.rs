//! `riku init` for the user running it, and the entry point that hands a
//! root caller to [`super::root_init`]. The output is one rail: the intro,
//! a step per stage, then the outro and summary from [`super::guidance`].

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::display;

use super::binary::require_root_binary;
use super::default_branch::{set_default_branch, BRANCH_ENV};
use super::deploy_keys::{add_deploy_keys, parse_key_args, read_key_file};
use super::deploy_user::running_as_root;
use super::git_hook::create_git_hook;
use super::guidance::{print_finish, InstallScope};
use super::init_router::persist_router;
use super::outside_supervisor::replace_outside_supervisor;
use super::prerequisites::{check_prerequisites, warn_not_root};
use super::root_init::root_init;
use super::router_choice;
use super::runtime_plugins::refresh_runtime_plugins;
use super::server_ip::{ensure_routing_section, FallbackDomain, SERVER_IP_HINT};
use super::ssh::setup_ssh_key_interactive;
use super::supervisor_binary::UnitScope;
use super::system_install::refuse_beside_system_install;
use super::systemd::setup_systemd_service;

fn create_directory_structure(paths: &RikuPaths, router: &Router) -> Result<()> {
    let mut dirs: Vec<&PathBuf> = vec![
        &paths.app_root,
        &paths.cache_root,
        &paths.data_root,
        &paths.git_root,
        &paths.env_root,
        &paths.workers_root,
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.log_root,
        &paths.acme_cert_links,
        &paths.plugin_root,
    ];
    // Caddy's site directory sits outside the home; root init creates it.
    if *router != Router::Caddy {
        dirs.push(&paths.nginx_root);
    }
    for dir in dirs {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("failed to create {}: {e}", dir.display()));
        }
    }
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    match env::var("HOME") {
        Ok(home) => Ok(PathBuf::from(home)),
        Err(_) => Err(anyhow!("HOME is not set")),
    }
}

fn default_branch_step() -> Result<String> {
    match home_dir() {
        Ok(home) => set_default_branch(&home, env::var(BRANCH_ENV).ok()),
        Err(e) => Err(e),
    }
}

/// `path` with `home` shown as `~`.
fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// The fallback domain step: on, or off with how to turn it on.
fn print_fallback(fallback: &FallbackDomain) {
    match fallback {
        FallbackDomain::On(ip) => {
            display::step_done("Fallback domain", &format!("on, server_ip {ip}"))
        }
        FallbackDomain::Off(why) => {
            display::warn("Fallback domain off");
            display::detail(&format!("{why}. {SERVER_IP_HINT}"));
        }
    }
}

/// Directories, git hook and default branch in `paths`' tree, reported as
/// one `Files` step.
fn prepare_tree(paths: &RikuPaths, router: &Router) -> Result<()> {
    if let Err(e) = create_directory_structure(paths, router) {
        return Err(e);
    }
    if let Err(e) = create_git_hook(paths) {
        return Err(e);
    }
    let branch = match default_branch_step() {
        Ok(branch) => branch,
        Err(e) => return Err(e),
    };
    let root = match home_dir() {
        Ok(home) => tilde(&paths.riku_root, &home),
        Err(_) => paths.riku_root.display().to_string(),
    };
    display::step_done("Files", &format!("{root}, git hook on branch {branch}"));
    Ok(())
}

fn setup_user_service(paths: &RikuPaths, router: &Router, no_systemd: bool) -> Result<()> {
    if no_systemd {
        display::step_done("Service", "skipped (--no-systemd)");
        return Ok(());
    }
    setup_systemd_service(paths, router)
}

/// Only root can create the uWSGI socket directory, so a plain init says
/// when it is missing instead of letting the first wsgi deploy fail.
fn warn_no_socket_root(paths: &RikuPaths) {
    if paths.socket_root.is_dir() {
        return;
    }
    display::warn(&format!("{} does not exist", paths.socket_root.display()));
    display::detail(
        "wsgi, jwsgi, rwsgi and php apps cannot deploy. Create it with: sudo riku init",
    );
}

/// What `riku init` was asked to do.
pub struct InitOptions {
    pub no_systemd: bool,
    /// Set by a root init when it runs the per-user steps as the deploy user.
    pub user_steps: bool,
    /// With `user_steps`, only authorize `add_keys`: set by
    /// `riku deploy-key add`, which must not touch the tree or the router.
    pub keys_only: bool,
    /// `--ssh-key`: a `.pub` or `authorized_keys` file whose keys to authorize.
    pub ssh_key: Option<PathBuf>,
    /// Keys a root init already read and validated, for the per-user run.
    pub add_keys: Vec<String>,
}

/// Authorize the keys given on the command line, or else prompt for one.
/// A root init gives the per-user run no terminal, so that run never
/// prompts; root prints how to add a key instead.
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
    if keys.is_empty() {
        setup_ssh_key_interactive()
    } else {
        add_deploy_keys(&keys)
    }
}

/// The router this init sets up. In the user steps of a root init, root has
/// already chosen and passed it as `RIKU_ROUTER`. A plain init chooses
/// itself; `fresh` is read before anything is created.
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

/// Init for the user running it, in that user's own tree; returns the
/// router it set up. With `user_steps`, a root init is driving: the
/// warning, prerequisites, systemd step and summary are root's to do.
fn user_init(paths: &RikuPaths, options: &InitOptions) -> Result<Router> {
    let user_steps = options.user_steps;
    if !user_steps {
        if let Err(e) = refuse_beside_system_install() {
            return Err(e);
        }
        if let Err(e) = require_root_binary() {
            return Err(e);
        }
        check_prerequisites();
    }
    let router = match init_router_choice(paths, user_steps) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if let Err(e) = persist_router(paths, &router) {
        return Err(e);
    }
    match ensure_routing_section(paths) {
        Ok(Some(fallback)) => print_fallback(&fallback),
        Ok(None) => {}
        Err(e) => return Err(e),
    }
    if !user_steps {
        warn_not_root(&router);
    }

    if let Err(e) = prepare_tree(paths, &router) {
        return Err(e);
    }
    refresh_runtime_plugins(paths);
    if user_steps {
        // Root starts the system unit next; only this deploy user child may
        // signal the deploy user's supervisor.
        if !options.no_systemd {
            if let Err(e) = replace_outside_supervisor(paths, UnitScope::System) {
                return Err(e);
            }
        }
    } else {
        if let Err(e) = setup_user_service(paths, &router, options.no_systemd) {
            return Err(e);
        }
        warn_no_socket_root(paths);
    }

    match ssh_key_step(options) {
        Ok(()) => Ok(router),
        Err(e) => Err(e),
    }
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
/// happens in the caller's own tree. The per-user run a root init starts
/// prints only its steps, so the two read as one sequence.
pub fn cmd_init(options: InitOptions) -> Result<()> {
    if options.user_steps && options.keys_only {
        return ssh_key_step(&options);
    }
    if !options.user_steps {
        display::intro(&format!("riku init  v{}", env!("CARGO_PKG_VERSION")));
    }
    if running_as_root() {
        let (user, router) = match root_init(&options) {
            Ok(done) => done,
            Err(e) => return Err(e),
        };
        let scope = InstallScope::System { user: user.name };
        print_finish(&scope, options.no_systemd, router.name());
        return Ok(());
    }

    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    let router = match user_init(&paths, &options) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if !options.user_steps {
        print_finish(&own_scope(), options.no_systemd, router.name());
    }
    Ok(())
}

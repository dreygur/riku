/// Server initialization: directory structure, git hook, systemd, SSH, and verification.
use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::echo;

use super::binary::{install_riku_binary, PathNote};
use super::default_branch::{set_default_branch, BRANCH_ENV};
use super::deploy_keys::{add_deploy_keys, parse_key_args, read_key_file};
use super::deploy_user::running_as_root;
use super::git_hook::create_git_hook;
use super::guidance::{print_finish, InstallScope};
use super::init_router::persist_router;
use super::prerequisites::{check_prerequisites, warn_not_root};
use super::root_init::root_init;
use super::ssh::setup_ssh_key_interactive;
use super::systemd::setup_systemd_service;

#[allow(clippy::question_mark)]
fn create_directory_structure(paths: &RikuPaths, router: &Router) -> Result<()> {
    let router_dir = match router {
        Router::Caddy => ("caddy", &paths.caddy_root),
        Router::Nginx | Router::Plugin(_) => ("nginx", &paths.nginx_root),
    };
    let dirs: Vec<(&str, &PathBuf)> = vec![
        ("apps", &paths.app_root),
        ("cache", &paths.cache_root),
        ("data", &paths.data_root),
        ("repos", &paths.git_root),
        ("envs", &paths.env_root),
        ("workers", &paths.workers_root),
        ("workers-available", &paths.workers_available),
        ("workers-enabled", &paths.workers_enabled),
        ("logs", &paths.log_root),
        router_dir,
        ("acme", &paths.acme_www),
        ("plugins", &paths.plugin_root),
    ];

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

fn generate_acme_config(paths: &RikuPaths) {
    if let Err(e) = crate::nginx::generate_acme_nginx_config(paths) {
        echo(
            &format!(
                "  ⚠ Could not generate ACME nginx config (nginx may not be installed): {}",
                e
            ),
            "yellow",
        );
    }
}
#[allow(clippy::question_mark)]
fn default_branch_step() -> Result<()> {
    let home = match env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => return Err(anyhow!("HOME is not set")),
    };
    set_default_branch(&home, env::var(BRANCH_ENV).ok())
}

/// Directories, binary, ACME config, git hook and default branch in
/// `paths`' tree.
#[allow(clippy::question_mark)]
fn prepare_tree(paths: &RikuPaths, router: &Router, note: PathNote) -> Result<()> {
    echo("[1/4] Creating directory structure...", "");
    if let Err(e) = create_directory_structure(paths, router) {
        return Err(e);
    }
    if let Err(e) = install_riku_binary(note) {
        return Err(e);
    }
    if *router != Router::Caddy {
        generate_acme_config(paths);
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

/// Init for the user running it, in that user's own tree. With
/// `user_steps`, a root init is driving: the warning, prerequisites and
/// systemd step are root's to do, and root prints the summary.
#[allow(clippy::question_mark)]
fn user_init(paths: &RikuPaths, options: &InitOptions) -> Result<()> {
    let user_steps = options.user_steps;
    let router = match persist_router(paths) {
        Ok(router) => router,
        Err(e) => return Err(e),
    };
    if !user_steps {
        warn_not_root(&router);
        check_prerequisites(false, &router);
        echo("-----> Initializing Riku server...", "");
        echo("", "");
    }

    let note = if user_steps {
        PathNote::Skip
    } else {
        PathNote::Show
    };
    if let Err(e) = prepare_tree(paths, &router, note) {
        return Err(e);
    }
    if !user_steps {
        if let Err(e) = setup_user_service(paths, &router, options.no_systemd) {
            return Err(e);
        }
    }

    ssh_key_step(options)
}

fn own_scope() -> InstallScope {
    InstallScope::User {
        user: env::var("USER").unwrap_or_default(),
        home: PathBuf::from(env::var("HOME").unwrap_or_default()),
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
        let scope = InstallScope::System {
            user: user.name,
            home: user.home,
        };
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

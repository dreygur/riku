/// Server initialization: directory structure, git hook, systemd, SSH, and verification.
use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::echo;

use super::binary::install_riku_binary;
use super::git_hook::create_git_hook;
use super::guidance::{print_first_app_guide, print_verification_summary, verify_supervisor};
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
/// Directories, binary, ACME config and git hook in `paths`' tree.
#[allow(clippy::question_mark)]
fn prepare_tree(paths: &RikuPaths, router: &Router) -> Result<()> {
    echo("[1/4] Creating directory structure...", "");
    if let Err(e) = create_directory_structure(paths, router) {
        return Err(e);
    }
    if let Err(e) = install_riku_binary() {
        return Err(e);
    }
    if *router != Router::Caddy {
        generate_acme_config(paths);
    }

    echo("", "");
    if let Err(e) = create_git_hook(paths) {
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

/// Init for the user running it, in that user's own tree. With
/// `user_steps`, a root init is driving: the warning, prerequisites and
/// systemd step are root's to do, and root prints the summary.
#[allow(clippy::question_mark)]
fn user_init(paths: &RikuPaths, no_systemd: bool, user_steps: bool) -> Result<()> {
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

    if let Err(e) = prepare_tree(paths, &router) {
        return Err(e);
    }
    if !user_steps {
        if let Err(e) = setup_user_service(paths, &router, no_systemd) {
            return Err(e);
        }
    }

    // A root init gives this run no terminal, so it cannot prompt for a key.
    if user_steps {
        return Ok(());
    }
    echo("[3/4] SSH key setup...", "");
    if let Err(e) = setup_ssh_key_interactive(paths) {
        return Err(e);
    }
    echo("", "");
    Ok(())
}

fn print_finish(no_systemd: bool) {
    echo("[4/4] Verifying installation...", "");
    verify_supervisor(no_systemd);
    echo("", "");

    echo("-----> Riku server initialized successfully!", "green");
    echo("", "");

    print_verification_summary(no_systemd);
    print_first_app_guide();
}

/// Initialize Riku on a server.
///
/// As root, the per-user steps run as the deploy user (see
/// [`super::root_init`]) and root adds the host setup. Otherwise everything
/// happens in the caller's own tree. `user_steps` is the hidden flag a root
/// init passes to that per-user run.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn cmd_init(no_systemd: bool, user_steps: bool) -> Result<()> {
    let is_root = env::var("USER").unwrap_or_default() == "root";
    if is_root {
        if let Err(e) = root_init(no_systemd) {
            return Err(e);
        }
        print_finish(no_systemd);
        return Ok(());
    }

    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    if let Err(e) = user_init(&paths, no_systemd, user_steps) {
        return Err(e);
    }
    if !user_steps {
        print_finish(no_systemd);
    }
    Ok(())
}

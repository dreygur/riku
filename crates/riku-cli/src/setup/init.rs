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
use super::init_router::{resolve_router, setup_router_host, RouterTarget};
use super::prerequisites::{check_prerequisites, warn_not_root};
use super::ssh::setup_ssh_key_interactive;
use super::systemd::{install_systemd_service, setup_systemd_service};

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
fn setup_systemd_step(
    paths: &RikuPaths,
    router: &Router,
    no_systemd: bool,
    is_root: bool,
) -> Result<()> {
    if no_systemd {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
        echo("", "");
        return Ok(());
    }

    echo("[2/4] Setting up systemd service...", "");
    let installed = if is_root {
        install_systemd_service(router)
    } else {
        setup_systemd_service(paths, router)
    };
    if let Err(e) = installed {
        return Err(e);
    }
    echo("", "");
    Ok(())
}

/// Directories, binary, router host setup and git hook: everything up to
/// the systemd step.
#[allow(clippy::question_mark)]
fn prepare_host(paths: &RikuPaths, target: &RouterTarget) -> Result<()> {
    echo("[1/4] Creating directory structure...", "");
    if let Err(e) = create_directory_structure(paths, &target.router) {
        return Err(e);
    }
    if let Err(e) = install_riku_binary() {
        return Err(e);
    }
    if target.router != Router::Caddy {
        generate_acme_config(paths);
    }
    if let Err(e) = setup_router_host(target) {
        return Err(e);
    }

    echo("", "");
    if let Err(e) = create_git_hook(paths) {
        return Err(e);
    }
    echo("", "");
    Ok(())
}

/// Initialize Riku on a server.
/// Creates directory structure, optionally sets up systemd, and configures SSH keys.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn cmd_init(no_systemd: bool) -> Result<()> {
    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };

    let is_root = env::var("USER").unwrap_or_default() == "root";
    let target = match resolve_router(&paths, is_root) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };

    if !is_root {
        warn_not_root(&target.router);
    }

    check_prerequisites(is_root, &target.router);

    echo("-----> Initializing Riku server...", "");
    echo("", "");

    if let Err(e) = prepare_host(&paths, &target) {
        return Err(e);
    }
    if let Err(e) = setup_systemd_step(&paths, &target.router, no_systemd, is_root) {
        return Err(e);
    }

    echo("[3/4] SSH key setup...", "");
    if let Err(e) = setup_ssh_key_interactive(&paths) {
        return Err(e);
    }
    echo("", "");

    echo("[4/4] Verifying installation...", "");
    verify_supervisor(no_systemd);
    echo("", "");

    echo("-----> Riku server initialized successfully!", "green");
    echo("", "");

    print_verification_summary(no_systemd);
    print_first_app_guide();

    Ok(())
}

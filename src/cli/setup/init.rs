/// Server initialization: directory structure, git hook, systemd, SSH, and verification.
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

use super::binary::install_riku_binary;
use super::git_hook::create_git_hook;
use super::guidance::{print_first_app_guide, print_verification_summary, verify_supervisor};
use super::ssh::setup_ssh_key_interactive;
use super::systemd::{install_systemd_service, setup_systemd_service};

/// Initialize Riku on a server.
/// Creates directory structure, optionally sets up systemd, and configures SSH keys.
pub fn cmd_init(no_systemd: bool) -> Result<()> {
    let paths = RikuPaths::from_env();

    let is_root = env::var("USER").unwrap_or_default() == "root";

    if !is_root {
        echo("⚠ Warning: Not running as root", "yellow");
        echo("  Some features require root privileges:", "yellow");
        echo("    - System-wide systemd service", "yellow");
        echo("    - Nginx configuration", "yellow");
        echo("  Run 'sudo riku init' for full installation.", "yellow");
        echo("", "");
    }

    check_prerequisites(is_root);

    echo("-----> Initializing Riku server...", "");
    echo("", "");

    echo("[1/4] Creating directory structure...", "");
    create_directory_structure(&paths)?;

    install_riku_binary()?;
    generate_acme_config(&paths);

    echo("", "");
    create_git_hook(&paths)?;
    echo("", "");

    setup_systemd_step(&paths, no_systemd, is_root)?;

    echo("[3/4] SSH key setup...", "");
    setup_ssh_key_interactive(&paths)?;
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

fn check_prerequisites(is_root: bool) {
    echo("Checking prerequisites...", "");
    let mut missing_deps = Vec::new();

    if Command::new("git").arg("--version").output().is_err() {
        missing_deps.push("git");
    }

    let has_nginx = Command::new("nginx").arg("-v").output().is_ok();
    if !has_nginx && is_root {
        echo("  ⚠ nginx not found - install for web serving", "yellow");
    }

    if !missing_deps.is_empty() {
        echo(
            &format!("  ⚠ Missing dependencies: {}", missing_deps.join(", ")),
            "yellow",
        );
        echo(
            &format!("  Install with: apt install {}", missing_deps.join(" ")),
            "yellow",
        );
    } else {
        echo("  ✓ All required dependencies found", "green");
    }
    echo("", "");
}

fn create_directory_structure(paths: &RikuPaths) -> Result<()> {
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
        ("nginx", &paths.nginx_root),
        ("acme", &paths.acme_www),
        ("plugins", &paths.plugin_root),
    ];

    for (name, dir) in &dirs {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
            echo(&format!("      ✓ ~/.riku/{}", name), "green");
        } else {
            echo(&format!("      ✓ ~/.riku/{} (exists)", name), "green");
        }
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

fn setup_systemd_step(paths: &RikuPaths, no_systemd: bool, is_root: bool) -> Result<()> {
    if !no_systemd {
        echo("[2/4] Setting up systemd service...", "");
        if is_root {
            install_systemd_service(paths)?;
        } else {
            setup_systemd_service(paths)?;
        }
        echo("", "");
    } else {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
        echo("", "");
    }
    Ok(())
}


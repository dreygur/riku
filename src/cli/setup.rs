use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, setup_authorized_keys};

#[allow(dead_code)]
/// Install systemd service files for riku daemon (system-wide, requires root)
fn install_systemd_service(_paths: &RikuPaths) -> Result<()> {
    // Check if we're running as root (required for systemd installation)
    if env::var("USER").unwrap_or_default() != "root" {
        return Ok(()); // Skip if not root
    }

    // Find the riku binary location
    let riku_binary = env::current_exe()?;
    let riku_binary_path = riku_binary.to_string_lossy();

    // Get the actual deploy user's home directory from system
    // Default to "deploy" but can be overridden by RIKU_USER env var
    let deploy_user = env::var("RIKU_USER").unwrap_or_else(|_| "deploy".to_string());
    let deploy_home = match Command::new("getent")
        .arg("passwd")
        .arg(&deploy_user)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let line = String::from_utf8_lossy(&output.stdout);
                line.split(':')
                    .nth(5)
                    .unwrap_or(&format!("/home/{}", deploy_user))
                    .to_string()
            } else {
                format!("/home/{}", deploy_user)
            }
        }
        Err(_) => format!("/home/{}", deploy_user),
    };

    // Create systemd directory if needed
    let systemd_dir = Path::new("/etc/systemd/system");
    if !systemd_dir.exists() {
        fs::create_dir_all(systemd_dir)?;
    }

    // Create riku.service file
    let service_content = format!(
        r#"[Unit]
Description=Riku Process Supervisor
Documentation=https://dreygur.github.io/riku/
After=network.target nginx.service
Wants=nginx.service

[Service]
Type=simple
User={deploy_user}
Group={deploy_user}
WorkingDirectory={deploy_home}
ExecStart={riku_bin} supervisor
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=riku

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

# Allow writing to riku directories
ReadWritePaths={deploy_home}/.riku

# Resource limits
MemoryMax=512M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
"#,
        deploy_user = deploy_user,
        deploy_home = deploy_home,
        riku_bin = riku_binary_path
    );

    let service_path = systemd_dir.join("riku.service");
    fs::write(&service_path, &service_content)?;
    echo(
        &format!("✓ Created systemd service at {}", service_path.display()),
        "green",
    );

    // Create nginx path watcher
    let nginx_path_content = format!(
        r#"[Unit]
Description=Watch for Riku nginx configuration changes
Documentation=https://dreygur.github.io/riku/
PartOf=riku.service

[Path]
PathModified={deploy_home}/.riku/nginx
Unit=riku-nginx-reload.service

[Install]
WantedBy=multi-user.target
"#,
        deploy_home = deploy_home
    );

    let nginx_path = systemd_dir.join("riku-nginx.path");
    fs::write(&nginx_path, nginx_path_content)?;

    // Create nginx reload service
    let nginx_reload_content = r#"[Unit]
Description=Reload nginx when Riku configuration changes
Documentation=https://dreygur.github.io/riku/

[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl reload nginx

[Install]
WantedBy=multi-user.target
"#;

    let nginx_reload = systemd_dir.join("riku-nginx-reload.service");
    fs::write(&nginx_reload, nginx_reload_content)?;

    // Reload systemd and enable service
    echo("Enabling riku systemd service...", "green");
    if let Ok(output) = Command::new("systemctl").arg("daemon-reload").output() {
        if output.status.success() {
            echo("✓ Reloaded systemd daemon", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl").args(["enable", "riku"]).output() {
        if output.status.success() {
            echo("✓ Enabled riku service (starts on boot)", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl").args(["start", "riku"]).output() {
        if output.status.success() {
            echo("✓ Started riku service", "green");

            // Verify supervisor is running
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(status) = Command::new("systemctl")
                .args(["is-active", "riku"])
                .output()
            {
                if String::from_utf8_lossy(&status.stdout).trim() == "active" {
                    echo("✓ Supervisor daemon is running", "green");
                }
            }
        }
    }

    // Enable nginx auto-reload
    if let Ok(output) = Command::new("systemctl")
        .args(["enable", "riku-nginx.path"])
        .output()
    {
        if output.status.success() {
            echo("✓ Enabled nginx auto-reload watcher", "green");
        }
    }

    if let Ok(output) = Command::new("systemctl")
        .args(["start", "riku-nginx.path"])
        .output()
    {
        if output.status.success() {
            echo("✓ Started nginx auto-reload watcher", "green");
        }
    }

    Ok(())
}

/// Install default nginx configuration
#[allow(dead_code)]
fn install_nginx_default_config() -> Result<()> {
    // Check if we're running as root (required for nginx configuration)
    if env::var("USER").unwrap_or_default() != "root" {
        return Ok(()); // Skip if not root
    }

    // Find the contrib/nginx/default.conf file
    // Try multiple possible locations
    let possible_paths = [
        "contrib/nginx/default.conf",
        "/usr/share/riku/nginx/default.conf",
        "/etc/riku/nginx/default.conf",
    ];

    let source_path = possible_paths
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|s| s.to_string());

    let source_path = match source_path {
        Some(p) => p,
        None => return Ok(()), // Skip if template not found
    };

    // Determine nginx config directory
    let nginx_conf_dir = if std::path::Path::new("/etc/nginx/sites-available").exists() {
        "/etc/nginx/sites-available"
    } else if std::path::Path::new("/etc/nginx/conf.d").exists() {
        "/etc/nginx/conf.d"
    } else {
        return Ok(()); // Skip if nginx not installed
    };

    let dest_path = std::path::Path::new(nginx_conf_dir).join("riku-default.conf");

    // Copy the default config
    echo("Installing default nginx configuration...", "green");
    fs::copy(&source_path, &dest_path)?;
    echo(&format!("✓ Created {}", dest_path.display()), "green");

    // Enable the config (for sites-available)
    if nginx_conf_dir.contains("sites-available") {
        let sites_enabled = std::path::Path::new("/etc/nginx/sites-enabled/riku-default.conf");
        if !sites_enabled.exists() {
            std::os::unix::fs::symlink(&dest_path, sites_enabled)?;
            echo("✓ Enabled nginx configuration", "green");
        }
    }

    // Test nginx configuration
    if let Ok(output) = Command::new("nginx").arg("-t").output() {
        if output.status.success() {
            echo("✓ Nginx configuration is valid", "green");

            // Reload nginx if it's running
            if let Ok(status) = Command::new("systemctl")
                .arg("is-active")
                .arg("nginx")
                .output()
            {
                if String::from_utf8_lossy(&status.stdout).trim() == "active"
                    && Command::new("systemctl")
                        .arg("reload")
                        .arg("nginx")
                        .output()
                        .is_ok()
                {
                    echo("✓ Reloaded nginx", "green");
                }
            }
        } else {
            echo("⚠ Nginx configuration test failed", "yellow");
        }
    }

    Ok(())
}

/// Install riku binary to user's PATH
fn install_riku_binary() -> Result<()> {
    // Get current executable path
    let current_exe = env::current_exe()?;

    // Determine installation target (always user-local, never requires root)
    let target_dir = get_user_install_directory()?;
    let target_path = target_dir.join("riku");

    // Check if already installed
    if target_path.exists() && current_exe == target_path {
        // Already installed in correct location
        return Ok(());
    }

    // Create target directory if it doesn't exist
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)?;
    }

    // Copy binary to target location
    echo(
        &format!("Installing riku to '{}'...", target_path.display()),
        "green",
    );
    fs::copy(&current_exe, &target_path)?;

    // Set executable permissions
    let mut perms = fs::metadata(&target_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_path, perms)?;

    echo(
        &format!("✓ Installed to {}", target_path.display()),
        "green",
    );

    // Check if target directory is in PATH
    if !is_in_path(&target_dir) {
        echo("", "");
        echo(
            &format!("⚠ Note: {} is not in your PATH.", target_dir.display()),
            "",
        );

        // Try to add to shell config automatically
        let shell_configs = [".bashrc", ".zshrc", ".profile"];
        let mut added = false;

        if let Ok(home) = env::var("HOME") {
            for config in &shell_configs {
                let config_path = PathBuf::from(&home).join(config);
                if config_path.exists() {
                    // Check if already in config
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if !content.contains(".local/bin") {
                            // Add to config
                            if let Ok(mut file) =
                                fs::OpenOptions::new().append(true).open(&config_path)
                            {
                                use std::io::Write;
                                let _ = writeln!(
                                    file,
                                    "\n# Add Riku to PATH\nexport PATH=\"$HOME/.local/bin:$PATH\""
                                );
                                added = true;
                                echo(&format!("✓ Added PATH export to ~/{}", config), "");
                                break;
                            }
                        }
                    }
                }
            }
        }

        if !added {
            echo(
                "Add it manually to your shell config (~/.bashrc, ~/.zshrc):",
                "",
            );
            echo("  export PATH=\"$HOME/.local/bin:$PATH\"", "");
        }

        echo("", "");
        echo("Or reload your shell, or use the full path:", "");
        echo(&format!("  {}", target_path.display()), "");
        echo("", "");
        echo("After adding to PATH, run: exec $SHELL -l", "");
    }

    Ok(())
}

/// Get user-local installation directory (~/.local/bin)
fn get_user_install_directory() -> Result<PathBuf> {
    // Always install to user-local directory (no root required)
    // Follows XDG Base Directory specification

    if let Ok(home) = env::var("HOME") {
        // Primary: ~/.local/bin (XDG standard)
        let local_bin = PathBuf::from(&home).join(".local/bin");
        return Ok(local_bin);
    }

    // Fallback: current directory
    if let Ok(cwd) = env::current_dir() {
        return Ok(cwd);
    }

    bail!("Could not determine home directory for installation")
}

/// Check if a directory is in the PATH environment variable
fn is_in_path(dir: &Path) -> bool {
    if let Ok(path) = env::var("PATH") {
        for path_dir in env::split_paths(&path) {
            if path_dir == dir {
                return true;
            }
        }
    }
    false
}

#[allow(dead_code)]
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Initialize Riku on a server.
/// Creates directory structure, optionally sets up systemd, and configures SSH keys.
pub fn cmd_init(no_systemd: bool) -> Result<()> {
    let paths = RikuPaths::from_env();

    // Check if running as root for full installation
    let is_root = env::var("USER").unwrap_or_default() == "root";

    if !is_root {
        echo("⚠ Warning: Not running as root", "yellow");
        echo("  Some features require root privileges:", "yellow");
        echo("    - System-wide systemd service", "yellow");
        echo("    - Nginx configuration", "yellow");
        echo("  Run 'sudo riku init' for full installation.", "yellow");
        echo("", "");
    }

    // Prerequisites check
    echo("Checking prerequisites...", "");
    let mut missing_deps = Vec::new();

    // Check git
    if Command::new("git").arg("--version").output().is_err() {
        missing_deps.push("git");
    }

    // Check nginx (warning only)
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

    echo("-----> Initializing Riku server...", "");
    echo("", "");

    // Step 1: Create directory structure
    echo("[1/4] Creating directory structure...", "");
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

    // Install riku binary to user's PATH
    install_riku_binary()?;
    echo("", "");

    // Create global post-receive hook template
    let hooks_dir = paths.git_root.parent().unwrap().join("hooks");
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir)?;
    }

    let post_receive = hooks_dir.join("post-receive");
    let hook_script = r#"#!/bin/bash
# Riku global post-receive hook
# This hook is called when code is pushed to any app repository

while read oldrev newrev refname; do
    # Extract app name from repository path
    APP=$(basename "$(pwd)" .git)

    # Run riku git-hook
    RIKU_BIN="$HOME/.local/bin/riku"
    if [ -x "$RIKU_BIN" ]; then
        "$RIKU_BIN" git-hook "$APP"
    else
        echo " !     Riku binary not found at $RIKU_BIN"
    fi
done
"#;

    fs::write(&post_receive, hook_script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&post_receive, fs::Permissions::from_mode(0o755))?;
    }

    echo("      ✓ Global git hook created", "green");
    echo("", "");

    // Step 2: Setup systemd (unless --no-systemd)
    if !no_systemd {
        echo("[2/4] Setting up systemd service...", "");
        setup_systemd_service(&paths)?;
        echo("", "");
    } else {
        echo("[2/4] Skipping systemd setup (--no-systemd)", "yellow");
        echo("", "");
    }

    // Step 3: SSH key setup
    echo("[3/4] SSH key setup...", "");
    setup_ssh_key_interactive(&paths)?;
    echo("", "");

    // Step 4: Verify installation
    echo("[4/4] Verifying installation...", "");

    if !no_systemd {
        let status = Command::new("systemctl")
            .args(["--user", "is-active", "riku"])
            .output();

        if let Ok(output) = status {
            if output.status.success() {
                echo("      ✓ Supervisor running", "green");
            } else {
                echo(
                    "      ⚠ Supervisor not running (start with: systemctl --user start riku)",
                    "yellow",
                );
            }
        }
    } else {
        echo(
            "      ℹ Supervisor not started (start manually with: riku supervisor)",
            "yellow",
        );
    }

    echo("", "");
    echo("-----> Riku server initialized successfully!", "green");
    echo("", "");

    // Post-init verification
    echo("Verification:", "green");

    // Check binary (user-local installation)
    if let Ok(home) = env::var("HOME") {
        let riku_path = PathBuf::from(&home).join(".local/bin/riku");
        if riku_path.exists() {
            echo(
                &format!("  ✓ Binary installed: {}", riku_path.display()),
                "green",
            );
        } else {
            echo(
                &format!("  ⚠ Binary not found: {}", riku_path.display()),
                "yellow",
            );
        }
    }

    // Check supervisor
    if !no_systemd {
        let status = Command::new("systemctl")
            .args(["--user", "is-active", "riku"])
            .output();

        if let Ok(output) = status {
            if output.status.success() {
                echo("  ✓ Supervisor running", "green");
            } else {
                echo(
                    "  ⚠ Supervisor not running (start with: systemctl --user start riku)",
                    "yellow",
                );
            }
        }
    } else {
        echo(
            "  ℹ Supervisor not started (start manually with: riku supervisor)",
            "yellow",
        );
    }

    echo("", "");

    // First app guide
    echo("Deploy your first app:", "green");
    echo("", "");
    echo("1. Create app directory on your local machine:", "yellow");
    echo("   mkdir myapp && cd myapp", "yellow");
    echo("   git init", "yellow");
    echo("", "");
    echo("2. Add your code and create a Procfile:", "yellow");
    echo("   echo 'web: python app.py' > Procfile", "yellow");
    echo("", "");
    echo("3. Deploy:", "yellow");
    echo(
        &format!(
            "   git remote add riku {}@your-server:myapp",
            env::var("USER").unwrap_or_else(|_| "deploy".to_string())
        ),
        "yellow",
    );
    echo("   git push riku main", "yellow");
    echo("", "");
    echo("Documentation: https://dreygur.github.io/riku/", "green");
    echo("", "");

    Ok(())
}

/// Setup systemd service for Riku supervisor.
fn setup_systemd_service(paths: &RikuPaths) -> Result<()> {
    // Create user systemd directory
    let systemd_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".config/systemd/user");

    fs::create_dir_all(&systemd_dir)?;

    // Get riku binary path (user-local, no root required)
    let riku_binary_path = paths.riku_script.to_string_lossy();

    // Create service file
    let service_content = format!(
        r#"[Unit]
Description=Riku Process Supervisor
Documentation=https://dreygur.github.io/riku/
After=network.target nginx.service
Wants=nginx.service

[Service]
Type=simple
ExecStart={riku_path} supervisor
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=riku

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

# Allow writing to riku directories
ReadWritePaths=~/.riku

# Resource limits
MemoryMax=512M
CPUQuota=50%

[Install]
WantedBy=default.target
"#,
        riku_path = riku_binary_path
    );

    // Create service file
    let service_file = systemd_dir.join("riku.service");
    fs::write(&service_file, &service_content)?;
    echo("      ✓ Service file created", "green");

    // Reload systemd daemon
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    // Enable service
    let status = Command::new("systemctl")
        .args(["--user", "enable", "riku"])
        .status();

    if status.is_ok_and(|s| s.success()) {
        echo("      ✓ Service enabled", "green");
    }

    // Start service
    let status = Command::new("systemctl")
        .args(["--user", "start", "riku"])
        .status();

    if status.is_ok_and(|s| s.success()) {
        echo("      ✓ Service started", "green");
    }

    Ok(())
}

/// Interactive SSH key setup.
fn setup_ssh_key_interactive(paths: &RikuPaths) -> Result<()> {
    let ssh_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".ssh");

    // Find existing public keys
    let mut found_keys = Vec::new();

    if ssh_dir.exists() {
        for entry in fs::read_dir(&ssh_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("pub") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if content.contains("ssh-") {
                        found_keys.push(path);
                    }
                }
            }
        }
    }

    let key_to_add = if found_keys.is_empty() {
        // No keys found, offer to create one
        echo("      ℹ No SSH keys found", "yellow");
        print!("      Create new SSH key? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            echo("      Generating SSH key...", "");

            let status = Command::new("ssh-keygen")
                .args([
                    "-t",
                    "ed25519",
                    "-C",
                    "riku@server",
                    "-f",
                    "~/.ssh/id_ed25519",
                    "-N",
                    "",
                ])
                .status();

            if status.is_ok_and(|s| s.success()) {
                echo("      ✓ SSH key created", "green");
                Some(ssh_dir.join("id_ed25519.pub"))
            } else {
                echo("      ⚠ Failed to create SSH key", "red");
                echo(
                    "      You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
                    "yellow",
                );
                None
            }
        } else {
            echo(
                "      ℹ You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
                "yellow",
            );
            None
        }
    } else if found_keys.len() == 1 {
        // Single key found, use it
        echo(
            &format!("      ✓ Found key: {}", found_keys[0].display()),
            "green",
        );
        Some(found_keys[0].clone())
    } else {
        // Multiple keys found, let user choose
        echo("      Multiple SSH keys found:", "yellow");

        for (i, key) in found_keys.iter().enumerate() {
            echo(&format!("        [{}] {}", i + 1, key.display()), "");
        }

        print!("      Select key (1-{}): ", found_keys.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= found_keys.len() => {
                echo(
                    &format!("      ✓ Selected: {}", found_keys[n - 1].display()),
                    "green",
                );
                Some(found_keys[n - 1].clone())
            }
            _ => {
                echo("      ⚠ Invalid selection", "red");
                echo(
                    "      You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
                    "yellow",
                );
                None
            }
        }
    };

    // Add selected key to authorized_keys
    if let Some(key_path) = key_to_add {
        if key_path.exists() {
            let pubkey = fs::read_to_string(&key_path)?.trim().to_string();

            // Get fingerprint
            let output = Command::new("ssh-keygen")
                .arg("-lf")
                .arg(&key_path)
                .output();

            let fingerprint = if let Ok(out) = output {
                String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string()
            } else {
                "unknown".to_string()
            };

            echo(
                &format!("      Adding key '{}' to authorized_keys...", fingerprint),
                "",
            );

            let script_path = paths.riku_script.to_string_lossy().to_string();
            setup_authorized_keys(&fingerprint, &script_path, &pubkey)?;

            echo("      ✓ Key added to authorized_keys", "green");
        }
    }

    Ok(())
}

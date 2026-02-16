use anyhow::{bail, Result};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, setup_authorized_keys};

/// Initialize the riku directory structure.
#[allow(dead_code)]
pub fn cmd_setup_init(paths: &RikuPaths) -> Result<()> {
    let dirs: Vec<&PathBuf> = vec![
        &paths.app_root,
        &paths.cache_root,
        &paths.data_root,
        &paths.git_root,
        &paths.env_root,
        &paths.workers_root,
        &paths.workers_available,
        &paths.workers_enabled,
        &paths.log_root,
        &paths.nginx_root,
    ];

    for dir in &dirs {
        if !dir.exists() {
            echo(&format!("Creating '{}'.", dir.display()), "green");
            fs::create_dir_all(dir)?;
        }
    }

    // Mark riku script as executable if it isn't already
    let script = &paths.riku_script;
    if script.exists() {
        let meta = fs::metadata(script)?;
        let mode = meta.permissions().mode();
        if mode & 0o100 == 0 {
            echo(
                &format!("Setting '{}' as executable.", script.display()),
                "yellow",
            );
            fs::set_permissions(script, fs::Permissions::from_mode(mode | 0o100))?;
        }
    }

    Ok(())
}

/// Set up a new SSH key. Use "-" for stdin.
#[allow(dead_code)]
pub fn cmd_setup_ssh(paths: &RikuPaths, public_key_file: &str) -> Result<()> {
    if public_key_file == "-" {
        // Read from stdin, write to a temp file, then process
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        let tmp_dir = std::env::temp_dir();
        let tmp_file = tmp_dir.join("piku_ssh_tmp_key.pub");
        fs::write(&tmp_file, &buffer)?;

        let result = add_ssh_key(paths, &tmp_file);
        let _ = fs::remove_file(&tmp_file);
        result
    } else {
        let key_path = PathBuf::from(public_key_file);
        if !key_path.exists() {
            echo(
                &format!("Error: public key file '{}' not found.", public_key_file),
                "red",
            );
            bail!("Public key file not found");
        }
        add_ssh_key(paths, &key_path)
    }
}

#[allow(dead_code)]
fn add_ssh_key(paths: &RikuPaths, key_file: &PathBuf) -> Result<()> {
    // Get fingerprint via ssh-keygen
    let output = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(key_file)
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        echo(
            &format!(
                "Error: invalid public key file '{}': {}",
                key_file.display(),
                err
            ),
            "red",
        );
        bail!("Invalid public key file");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let fingerprint = stdout.split_whitespace().nth(1).unwrap_or("").to_string();

    let key = fs::read_to_string(key_file)?.trim().to_string();

    echo(&format!("Adding key '{}'.", fingerprint), "");

    let script_path = paths.riku_script.to_string_lossy().to_string();
    setup_authorized_keys(&fingerprint, &script_path, &key)?;

    Ok(())
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
    echo("Your server is ready to receive deployments.", "green");
    echo("", "");
    echo("From your local machine:", "");
    echo("  git remote add riku deploy@your-server:myapp", "");
    echo("  git push riku master", "");
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

    // Get riku binary path
    let riku_binary = paths.riku_script.to_string_lossy();

    // Create service file
    let service_content = format!(
        r#"[Unit]
Description=Riku Server
Documentation=https://github.com/dreygur/riku
After=network.target

[Service]
Type=simple
ExecStart={riku_binary} supervisor
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=riku

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=default.target
"#,
        riku_binary = riku_binary
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

    if status.map_or(false, |s| s.success()) {
        echo("      ✓ Service enabled", "green");
    }

    // Start service
    let status = Command::new("systemctl")
        .args(["--user", "start", "riku"])
        .status();

    if status.map_or(false, |s| s.success()) {
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

            if status.map_or(false, |s| s.success()) {
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

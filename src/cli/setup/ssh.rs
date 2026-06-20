/// Interactive SSH key setup: discover, select, or generate keys and add to authorized_keys.
use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::{echo, setup_authorized_keys};

/// Interactive SSH key setup.
pub fn setup_ssh_key_interactive(paths: &RikuPaths) -> Result<()> {
    let ssh_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".ssh");

    let found_keys = find_public_keys(&ssh_dir)?;

    let key_to_add = select_key(found_keys, &ssh_dir)?;

    if let Some(key_path) = key_to_add {
        let (scope, apps) = prompt_scope_and_apps()?;
        add_key_to_authorized_keys(paths, &key_path, scope.as_deref(), apps.as_deref())?;
    }

    Ok(())
}

/// Add an SSH public key to authorized_keys from an explicit path
/// (non-interactive entry point for `riku setup ssh <pubkey>`).
pub fn cmd_setup_ssh(
    paths: &RikuPaths,
    pubkey_path: &str,
    scope: Option<&str>,
    apps: Option<&[String]>,
) -> Result<()> {
    let key_path = PathBuf::from(pubkey_path);
    if !key_path.exists() {
        anyhow::bail!("SSH public key not found: {}", pubkey_path);
    }
    add_key_to_authorized_keys(paths, &key_path, scope, apps)
}

/// Ask whether to restrict the key, and if so, to which scope tier and apps.
/// Returns `(None, None)` for unrestricted (full) access.
fn prompt_scope_and_apps() -> Result<(Option<String>, Option<Vec<String>>)> {
    print!("      Restrict this key to specific app(s)? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        return Ok((None, None));
    }

    print!("      Scope tier (readonly/staging/production): ");
    io::stdout().flush()?;
    let mut scope = String::new();
    io::stdin().read_line(&mut scope)?;
    let scope = scope.trim().to_lowercase();

    print!("      App name(s), comma-separated: ");
    io::stdout().flush()?;
    let mut apps = String::new();
    io::stdin().read_line(&mut apps)?;
    let apps: Vec<String> = apps
        .split(',')
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();

    Ok((Some(scope), Some(apps)))
}

fn find_public_keys(ssh_dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut found_keys = Vec::new();

    if ssh_dir.exists() {
        for entry in fs::read_dir(ssh_dir)? {
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

    Ok(found_keys)
}

fn select_key(found_keys: Vec<PathBuf>, ssh_dir: &std::path::Path) -> Result<Option<PathBuf>> {
    if found_keys.is_empty() {
        return prompt_create_key(ssh_dir);
    }

    if found_keys.len() == 1 {
        echo(
            &format!("      ✓ Found key: {}", found_keys[0].display()),
            "green",
        );
        return Ok(Some(found_keys[0].clone()));
    }

    prompt_select_key(found_keys)
}

fn prompt_create_key(ssh_dir: &std::path::Path) -> Result<Option<PathBuf>> {
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
            return Ok(Some(ssh_dir.join("id_ed25519.pub")));
        } else {
            echo("      ⚠ Failed to create SSH key", "red");
            echo(
                "      You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
                "yellow",
            );
        }
    } else {
        echo(
            "      ℹ You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
            "yellow",
        );
    }

    Ok(None)
}

fn prompt_select_key(found_keys: Vec<PathBuf>) -> Result<Option<PathBuf>> {
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
            Ok(Some(found_keys[n - 1].clone()))
        }
        _ => {
            echo("      ⚠ Invalid selection", "red");
            echo(
                "      You can add a key manually later with: riku setup ssh ~/.ssh/id_rsa.pub",
                "yellow",
            );
            Ok(None)
        }
    }
}

fn add_key_to_authorized_keys(
    paths: &RikuPaths,
    key_path: &PathBuf,
    scope: Option<&str>,
    apps: Option<&[String]>,
) -> Result<()> {
    if !key_path.exists() {
        return Ok(());
    }

    let pubkey = fs::read_to_string(key_path)?.trim().to_string();

    let output = Command::new("ssh-keygen").arg("-lf").arg(key_path).output();

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
    setup_authorized_keys(&fingerprint, &script_path, &pubkey, scope, apps)?;

    if let Some(scope) = scope {
        echo(
            &format!(
                "      ✓ Key added to authorized_keys (scope: {}, apps: {})",
                scope,
                apps.map(|a| a.join(",")).unwrap_or_default()
            ),
            "green",
        );
    } else {
        echo(
            "      ✓ Key added to authorized_keys (full access)",
            "green",
        );
    }

    Ok(())
}

/// Interactive SSH key setup: discover, select, or generate keys and add to authorized_keys.
use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::util::echo;

use super::deploy_keys::{add_deploy_keys, read_key_file};

const ADD_KEY_LATER: &str = "      ℹ Add a key later by running riku init again";

/// Interactive SSH key setup: pick or create one of this user's keys and
/// authorize it, through the same validation as `riku init --ssh-key`.
#[allow(clippy::question_mark)]
pub fn setup_ssh_key_interactive() -> Result<()> {
    let Some(home) = dirs::home_dir() else {
        return Err(anyhow::anyhow!("Could not determine home directory"));
    };
    let ssh_dir = home.join(".ssh");

    let found_keys = match find_public_keys(&ssh_dir) {
        Ok(keys) => keys,
        Err(e) => return Err(e),
    };
    let key_path = match select_key(found_keys, &ssh_dir) {
        Ok(Some(path)) => path,
        Ok(None) => return Ok(()),
        Err(e) => return Err(e),
    };
    match read_key_file(&key_path) {
        Ok(keys) => add_deploy_keys(&keys),
        Err(e) => Err(e),
    }
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

/// Flush the prompt and read one line of the answer.
fn read_answer() -> Result<String> {
    if let Err(e) = io::stdout().flush() {
        return Err(anyhow::anyhow!("failed to show the prompt: {e}"));
    }
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => Ok(input),
        Err(e) => Err(anyhow::anyhow!("failed to read the answer: {e}")),
    }
}

#[allow(clippy::question_mark)]
fn prompt_create_key(ssh_dir: &std::path::Path) -> Result<Option<PathBuf>> {
    echo("      ℹ No SSH keys found", "yellow");
    print!("      Create new SSH key? [y/N]: ");
    let input = match read_answer() {
        Ok(input) => input,
        Err(e) => return Err(e),
    };

    if input.trim().to_lowercase() == "y" {
        echo("      Generating SSH key...", "");

        if let Err(e) = fs::create_dir_all(ssh_dir) {
            return Err(anyhow::anyhow!(
                "failed to create {}: {e}",
                ssh_dir.display()
            ));
        }
        // No shell runs this, so the path must be absolute: a literal `~`
        // would make ssh-keygen write into a directory named `~`.
        let status = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-C", "riku@server", "-N", "", "-f"])
            .arg(ssh_dir.join("id_ed25519"))
            .status();

        if status.is_ok_and(|s| s.success()) {
            echo("      ✓ SSH key created", "green");
            return Ok(Some(ssh_dir.join("id_ed25519.pub")));
        } else {
            echo("      ⚠ Failed to create SSH key", "red");
            echo(ADD_KEY_LATER, "yellow");
        }
    } else {
        echo(ADD_KEY_LATER, "yellow");
    }

    Ok(None)
}

#[allow(clippy::question_mark)]
fn prompt_select_key(found_keys: Vec<PathBuf>) -> Result<Option<PathBuf>> {
    echo("      Multiple SSH keys found:", "yellow");

    for (i, key) in found_keys.iter().enumerate() {
        echo(&format!("        [{}] {}", i + 1, key.display()), "");
    }

    print!("      Select key (1-{}): ", found_keys.len());
    let input = match read_answer() {
        Ok(input) => input,
        Err(e) => return Err(e),
    };

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
            echo(ADD_KEY_LATER, "yellow");
            Ok(None)
        }
    }
}

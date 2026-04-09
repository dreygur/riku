use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};

use crate::config::{RikuPaths, RIKU_RAW_SOURCE_URL};
use crate::supervisor::Supervisor;
use crate::util::{display, exit_if_invalid, parse_settings};

/// Run a command in the app context with LIVE_ENV loaded.
pub fn cmd_run(paths: &RikuPaths, app: &str, cmd: &[String]) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    let config_file = paths.env_root.join(&app).join("LIVE_ENV");
    let mut env = HashMap::new();
    parse_settings(&config_file, &mut env)?;

    let app_dir = paths.app_root.join(&app);

    if cmd.is_empty() {
        anyhow::bail!("no command specified for 'riku run'");
    }

    // Exec the binary directly rather than rejoining into `sh -c` to avoid
    // shell injection via metacharacters in the command arguments.
    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(&app_dir)
        .envs(&env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.wait()?;
    Ok(())
}

/// Restart an app: stop then spawn.
pub fn cmd_restart(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    display::info(&format!("restarting app '{}'...", app));
    do_stop(paths, &app);
    // Trigger a deploy to restart the app
    let deltas: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    crate::deploy::do_deploy(&app, paths, &deltas, None)
}

/// Stop an app by removing enabled worker configs.
pub fn cmd_stop(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;
    do_stop(paths, &app);
    Ok(())
}

/// Self-update the binary by downloading latest from RIKU_RAW_SOURCE_URL.
pub fn cmd_update() -> Result<()> {
    display::info("Updating riku...");

    let output = Command::new("curl")
        .args([
            "-sL",
            "-w",
            "%{http_code}",
            RIKU_RAW_SOURCE_URL,
            "-o",
            "/dev/null",
        ])
        .output()?;

    let http_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if http_code == "200" {
        display::warn("Note: self-update for riku binary is not yet implemented.");
        display::note("The reference source is accessible.");
    } else {
        display::note(&format!(
            "Error updating riku - please check if {} is accessible from this machine.",
            RIKU_RAW_SOURCE_URL
        ));
    }
    display::success("Done.");
    Ok(())
}

/// Start the supervisor daemon.
/// Note: For production use, use 'riku supervisor --daemon' or systemd service.
pub fn cmd_supervisor(paths: &RikuPaths) -> Result<()> {
    let mut supervisor = Supervisor::new(paths.workers_enabled.clone())?;
    supervisor.run()
}

/// Hot reload an app (zero downtime restart).
pub fn cmd_hot_reload(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = exit_if_invalid(app, &paths.app_root)?;

    display::info(&format!("Hot reloading app '{}'...", app));

    // Signal the supervisor by updating the mtime of each enabled worker TOML.
    // The supervisor's file watcher (notify) detects the Modify event and reloads the config.
    // We achieve a real mtime bump by reading the content and writing it back.
    let toml_pattern = paths.workers_enabled.join(format!("{}-*.toml", app));

    if let Ok(entries) = glob::glob(toml_pattern.to_str().unwrap_or("")) {
        let mut count = 0;
        for entry in entries.flatten() {
            // Read and rewrite the file to bump its mtime, triggering a supervisor reload.
            match fs::read_to_string(&entry) {
                Ok(content) => {
                    if let Err(e) = fs::write(&entry, content) {
                        display::warn(&format!(
                            "Warning: failed to touch {}: {}",
                            entry.display(),
                            e
                        ));
                    } else {
                        count += 1;
                    }
                }
                Err(e) => {
                    display::warn(&format!(
                        "Warning: failed to read {}: {}",
                        entry.display(),
                        e
                    ));
                }
            }
        }

        if count > 0 {
            display::success(&format!("Triggered hot reload for {} worker(s)", count));
            display::warn("Note: Supervisor must be running for hot reload to take effect.");
        } else {
            display::warn("No worker configs found. Is the app deployed?");
        }
    }

    Ok(())
}

/// Stop an app by removing its enabled worker config files.
fn do_stop(paths: &RikuPaths, app: &str) {
    let mut configs: Vec<std::path::PathBuf> = Vec::new();

    for ext in &["ini", "toml"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                configs.push(entry);
            }
        }
    }

    if !configs.is_empty() {
        display::info(&format!("Stopping app '{}'...", app));
        for c in &configs {
            if let Err(e) = fs::remove_file(c) {
                tracing::warn!("Could not remove worker config {:?}: {}", c, e);
            }
        }
    } else {
        display::error(&format!("Error: app '{}' not deployed!", app));
    }
}

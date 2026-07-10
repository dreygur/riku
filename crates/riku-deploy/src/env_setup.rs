//! Environment variable setup and LIVE_ENV writing for deployed apps.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::RikuPaths;
use crate::error::DeployError;
use crate::plugins::executor::{classify_resource_exit, exit_code_for, tee_output};
use crate::supervisor::resource_limits::ResourceLimits;
use crate::util::echo;

/// Run the Procfile `preflight` command (if present).
///
/// Exits the process with the command's exit code on failure, matching
/// the behaviour expected by the PaaS deploy pipeline. If the command was
/// terminated by an enforced resource limit (OOM-killed, RLIMIT_CPU, or its
/// own allocator hitting RLIMIT_AS) prints a structured diagnostic instead
/// of a bare exit code.
pub fn run_preflight(preflight_cmd: &str, app_path: &Path) {
    echo("-----> Running preflight.", "green");
    let limits = ResourceLimits::from_env();

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(preflight_cmd)
        .current_dir(app_path)
        // Piped (not inherited) so a resource-exhaustion failure can be
        // classified from the captured stderr tail, while tee_output still
        // mirrors both streams live to the terminal.
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(move || limits.apply());
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            echo(&format!("-----> preflight command error: {}", e), "red");
            std::process::exit(1);
        }
    };
    let (tee_handles, stderr_tail) = tee_output(&mut child);
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            echo(&format!("-----> preflight command error: {}", e), "red");
            std::process::exit(1);
        }
    };
    for h in tee_handles {
        let _ = h.join();
    }

    if status.success() {
        return;
    }

    let tail = stderr_tail.lock().unwrap().clone();
    if let Some(cause) = classify_resource_exit(&status, &tail) {
        echo(
            &DeployError::resource_exhausted("preflight", preflight_cmd, &cause).to_string(),
            "red",
        );
        std::process::exit(exit_code_for(&status));
    }

    let code = exit_code_for(&status);
    echo(
        &format!(
            "-----> Exiting due to preflight command error value: {}",
            code
        ),
        "",
    );
    std::process::exit(code);
}

/// Run the Procfile `release` command (if present).
///
/// Exits the process with the command's exit code on failure. If the
/// command was terminated by an enforced resource limit (OOM-killed,
/// RLIMIT_CPU, or its own allocator hitting RLIMIT_AS) prints a structured
/// diagnostic instead of a bare exit code.
pub fn run_release(release_cmd: &str, app_path: &Path) -> Result<()> {
    echo("-----> Releasing", "green");
    let limits = ResourceLimits::from_env();
    let output = unsafe {
        Command::new("sh")
            .arg("-c")
            .arg(release_cmd)
            .current_dir(app_path)
            .pre_exec(move || limits.apply())
            .output()?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(cause) = classify_resource_exit(&output.status, &stderr) {
            echo(
                &DeployError::resource_exhausted("release", release_cmd, &cause).to_string(),
                "red",
            );
            std::process::exit(exit_code_for(&output.status));
        }

        let code = exit_code_for(&output.status);
        echo(
            &format!(
                "Error: Release command failed with exit code {}: {}",
                code, stderr
            ),
            "red",
        );
        std::process::exit(code);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        echo(&format!("Release output: {}", stdout.trim()), "green");
    }
    Ok(())
}

/// Write `LIVE_ENV` file with resolved environment variables for the app.
///
/// The file contains core riku variables plus everything from the app's
/// `ENV` file, so that the supervisor and workers have a stable snapshot.
pub fn write_live_env(app: &str, paths: &RikuPaths, env: &HashMap<String, String>) -> Result<()> {
    let live_env_path = paths.env_root.join(app).join("LIVE_ENV");
    let mut content = String::new();
    content.push_str(&format!("APP={}\n", app));
    content.push_str(&format!("LOG_ROOT={}\n", paths.log_root.display()));
    content.push_str(&format!(
        "DATA_ROOT={}\n",
        paths.data_root.join(app).display()
    ));
    if let Ok(home) = std::env::var("HOME") {
        content.push_str(&format!("HOME={}\n", home));
    }
    if let Ok(user) = std::env::var("USER") {
        content.push_str(&format!("USER={}\n", user));
    }

    let env_file = paths.env_root.join(app).join("ENV");
    if env_file.exists() {
        let env_content = fs::read_to_string(&env_file)?;
        for line in env_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                content.push_str(&format!("{}\n", line));
            }
        }
    }

    // Include any in-memory env vars not yet persisted to ENV file
    for (k, v) in env {
        content.push_str(&format!("{}={}\n", k, v));
    }

    fs::write(&live_env_path, &content)?;
    Ok(())
}

/// Write updated environment variables to the app's ENV file, then trigger a redeploy.
///
/// This is the service-layer entry point for config changes. CLI commands must
/// call this instead of wiring `write_config` + `do_deploy` themselves.
pub fn update_env_and_redeploy(
    app: &str,
    paths: &crate::config::RikuPaths,
    env: &HashMap<String, String>,
) -> Result<()> {
    let config_file = paths.env_root.join(app).join("ENV");
    crate::util::write_config(&config_file, env, "=")?;
    let deltas = HashMap::new();
    super::do_deploy(app, paths, &deltas, None)
}

/// Inject WSGI socket variables into `env` and persist them to the ENV file.
///
/// This must happen before a WSGI nginx config is generated so that the
/// config template sees `NGINX_WSGI` and `UWSGI_SOCKET`.
#[cfg(test)]
pub fn setup_wsgi_env(
    app: &str,
    paths: &RikuPaths,
    env: &mut HashMap<String, String>,
) -> Result<()> {
    let socket_path = paths.nginx_root.join(format!("{}.sock", app));
    env.insert("NGINX_WSGI".to_string(), "true".to_string());
    env.insert(
        "UWSGI_SOCKET".to_string(),
        socket_path.to_string_lossy().to_string(),
    );
    env.insert(
        "SOCKET".to_string(),
        format!("unix://{}", socket_path.to_string_lossy()),
    );

    let env_dir = paths.env_root.join(app);
    fs::create_dir_all(&env_dir)?;
    let env_file = env_dir.join("ENV");
    let mut env_content = if env_file.exists() {
        fs::read_to_string(&env_file)?
    } else {
        String::new()
    };
    if !env_content.contains("NGINX_WSGI") {
        env_content.push_str("NGINX_WSGI=true\n");
        env_content.push_str(&format!("UWSGI_SOCKET={}\n", socket_path.display()));
        fs::write(&env_file, &env_content)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "env_setup_tests.rs"]
mod tests;

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
#[allow(dead_code)]
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        crate::config::RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())
    }

    fn setup_env_dir(paths: &RikuPaths, app: &str) {
        std::fs::create_dir_all(paths.env_root.join(app)).unwrap();
        std::fs::create_dir_all(&paths.nginx_root).unwrap();
    }

    // --- write_live_env ---

    #[test]
    fn test_write_live_env_creates_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        let env = HashMap::new();
        write_live_env("myapp", &paths, &env)?;

        let live_env_path = paths.env_root.join("myapp").join("LIVE_ENV");
        assert!(live_env_path.exists());
        Ok(())
    }

    #[test]
    fn test_write_live_env_contains_app_name() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        let env = HashMap::new();
        write_live_env("myapp", &paths, &env)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
        assert!(content.contains("APP=myapp"));
        Ok(())
    }

    #[test]
    fn test_write_live_env_includes_in_memory_vars() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        let mut env = HashMap::new();
        env.insert(
            "DATABASE_URL".to_string(),
            "postgres://localhost/db".to_string(),
        );
        write_live_env("myapp", &paths, &env)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
        assert!(content.contains("DATABASE_URL=postgres://localhost/db"));
        Ok(())
    }

    #[test]
    fn test_write_live_env_reads_existing_env_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        // Write an ENV file ahead of time
        let env_file = paths.env_root.join("myapp").join("ENV");
        fs::write(&env_file, "SECRET_KEY=abc123\n")?;

        let env = HashMap::new();
        write_live_env("myapp", &paths, &env)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
        assert!(content.contains("SECRET_KEY=abc123"));
        Ok(())
    }

    #[test]
    fn test_write_live_env_skips_comment_lines_in_env_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        let env_file = paths.env_root.join("myapp").join("ENV");
        fs::write(&env_file, "# This is a comment\nFOO=bar\n")?;

        let env = HashMap::new();
        write_live_env("myapp", &paths, &env)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
        assert!(!content.contains("# This is a comment"));
        assert!(content.contains("FOO=bar"));
        Ok(())
    }

    #[test]
    fn test_write_live_env_contains_log_root() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "myapp");

        let env = HashMap::new();
        write_live_env("myapp", &paths, &env)?;

        let content = fs::read_to_string(paths.env_root.join("myapp").join("LIVE_ENV"))?;
        assert!(content.contains("LOG_ROOT="));
        Ok(())
    }

    // --- setup_wsgi_env ---

    #[test]
    fn test_setup_wsgi_env_sets_nginx_wsgi() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "wsgiapp");

        let mut env = HashMap::new();
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;

        assert_eq!(env.get("NGINX_WSGI").map(|s| s.as_str()), Some("true"));
        Ok(())
    }

    #[test]
    fn test_setup_wsgi_env_sets_uwsgi_socket() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "wsgiapp");

        let mut env = HashMap::new();
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;

        let socket = env.get("UWSGI_SOCKET").expect("UWSGI_SOCKET must be set");
        assert!(
            socket.contains("wsgiapp.sock"),
            "Socket path should contain app name"
        );
        Ok(())
    }

    #[test]
    fn test_setup_wsgi_env_sets_socket_with_unix_prefix() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "wsgiapp");

        let mut env = HashMap::new();
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;

        let socket = env.get("SOCKET").expect("SOCKET must be set");
        assert!(
            socket.starts_with("unix://"),
            "SOCKET should have unix:// prefix"
        );
        Ok(())
    }

    #[test]
    fn test_setup_wsgi_env_persists_to_env_file() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "wsgiapp");

        let mut env = HashMap::new();
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;

        let env_file = paths.env_root.join("wsgiapp").join("ENV");
        assert!(env_file.exists(), "ENV file should be created");
        let content = fs::read_to_string(&env_file)?;
        assert!(content.contains("NGINX_WSGI=true"));
        Ok(())
    }

    #[test]
    fn test_setup_wsgi_env_idempotent() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        setup_env_dir(&paths, "wsgiapp");

        let mut env = HashMap::new();
        // Call twice — should not duplicate the lines in the ENV file
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;
        setup_wsgi_env("wsgiapp", &paths, &mut env)?;

        let env_file = paths.env_root.join("wsgiapp").join("ENV");
        let content = fs::read_to_string(&env_file)?;
        let count = content.matches("NGINX_WSGI=true").count();
        assert_eq!(count, 1, "NGINX_WSGI=true should appear exactly once");
        Ok(())
    }
}

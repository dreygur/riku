//! Generic worker configuration creation for deployed apps.
//!
//! Handles Procfile parsing and worker config generation.
//! Scaling delta logic lives in `super::scaling`.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::echo;

pub(crate) use super::scaling::apply_scaling_deltas;

const UWSGI_PROCESSES: &str = "4";
const UWSGI_THREADS: &str = "4";
const NGINX_EXTERNAL_PORT: &str = "80";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read the scaling count for a given process kind from the SCALING file.
///
/// Returns 1 if the file doesn't exist or the kind isn't listed.
pub fn read_scaling_count(paths: &RikuPaths, app: &str, kind: &str) -> Result<u32> {
    let scaling_path = paths.env_root.join(app).join("SCALING");
    if !scaling_path.exists() {
        return Ok(1);
    }
    let content = fs::read_to_string(&scaling_path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            let val = line[pos + 1..].trim();
            if key == kind {
                if let Ok(n) = val.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(1)
}

/// Create worker configs for every process entry in the app's Procfile.
///
/// `start_cmd` is an optional fallback command supplied by the runtime plugin
/// via its `start` subcommand. It is used only when a Procfile entry has an
/// empty command.
pub fn create_workers_generic(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    start_cmd: Option<&str>,
) -> Result<()> {
    if should_restart(env) {
        remove_stale_configs(app, paths);
    }

    let entries = match parse_procfile(app_path)? {
        Some(e) => e,
        None => return Ok(()),
    };

    for (kind, command) in &entries {
        let effective_cmd = if command.is_empty() {
            start_cmd.unwrap_or(command.as_str())
        } else {
            command.as_str()
        };
        let count = read_scaling_count(paths, app, kind)?;
        for i in 1..=count {
            let worker_env = build_worker_env(app, kind, effective_cmd, env, paths)?;
            write_worker_config(app, app_path, kind, effective_cmd, i, worker_env, paths)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns true when `RIKU_AUTO_RESTART` is not explicitly disabled.
fn should_restart(env: &HashMap<String, String>) -> bool {
    env.get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true)
}

/// Remove existing worker symlinks from `workers_enabled` to trigger a restart.
///
/// Uses `"{app}-*"` not `"{app}*"` to avoid touching configs for apps
/// whose names share a prefix (e.g. "foo" would otherwise match "foobar").
fn remove_stale_configs(app: &str, paths: &RikuPaths) {
    for ext in &["toml", "ini"] {
        let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                if let Err(e) = fs::remove_file(&entry) {
                    tracing::warn!("Could not remove stale worker config {:?}: {}", entry, e);
                }
            }
        }
    }
}

/// Parse the Procfile at `app_path/Procfile` into `(kind, command)` pairs.
///
/// Returns `None` (and prints a warning) when no Procfile is found.
/// Comment lines and blank lines are skipped.
fn parse_procfile(app_path: &Path) -> Result<Option<Vec<(String, String)>>> {
    let procfile_path = app_path.join("Procfile");
    if !procfile_path.exists() {
        echo("-----> No Procfile found, skipping process creation", "yellow");
        return Ok(None);
    }

    let content = fs::read_to_string(&procfile_path)?;
    let entries = content
        .lines()
        .filter_map(parse_procfile_line)
        .collect();

    Ok(Some(entries))
}

/// Parse a single Procfile line into `(kind, command)`, or `None` for blank/comment lines.
fn parse_procfile_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let pos = line.find(':')?;
    let kind = line[..pos].trim().to_string();
    let command = line[pos + 1..].trim().to_string();
    Some((kind, command))
}

/// Returns true when `kind` uses a WSGI unix socket (wsgi, jwsgi, rwsgi, php).
fn is_wsgi_kind(kind: &str) -> bool {
    matches!(kind, "wsgi" | "jwsgi" | "rwsgi" | "php")
}

/// Returns true when `kind` needs nginx wiring (web + all wsgi variants).
fn is_web_facing(kind: &str) -> bool {
    kind == "web" || is_wsgi_kind(kind)
}

/// Build the environment map for a single worker instance.
///
/// Web-facing processes get PORT/SOCKET injected; wsgi variants get uwsgi vars.
/// Also persists nginx settings back to the app's ENV file.
fn build_worker_env(
    app: &str,
    kind: &str,
    command: &str,
    base_env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<HashMap<String, String>> {
    let mut env = base_env.clone();

    if !is_web_facing(kind) {
        return Ok(env);
    }

    let socket_path = paths.nginx_root.join(format!("{}.sock", app));

    if is_wsgi_kind(kind) {
        configure_wsgi_env(&socket_path, &mut env);
    } else {
        configure_web_env(&socket_path, &mut env, paths)?;
    }

    persist_nginx_env(app, kind, command, &socket_path, &env, paths)?;

    Ok(env)
}

/// Inject uwsgi unix-socket variables into the environment.
fn configure_wsgi_env(socket_path: &Path, env: &mut HashMap<String, String>) {
    env.insert("SOCKET".to_string(), format!("unix://{}", socket_path.to_string_lossy()));
    env.insert("UWSGI_SOCKET".to_string(), socket_path.to_string_lossy().to_string());
    env.insert("NGINX_WSGI".to_string(), "true".to_string());
    env.insert("UWSGI_PROCESSES".to_string(), UWSGI_PROCESSES.to_string());
    env.insert("UWSGI_THREADS".to_string(), UWSGI_THREADS.to_string());
}

/// Allocate a free TCP port and inject nginx port-map variables into the environment.
fn configure_web_env(
    socket_path: &Path,
    env: &mut HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::util::get_free_port;

    let port = get_free_port("127.0.0.1")?;
    env.insert("PORT".to_string(), port.to_string());
    env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
    env.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
    env.insert("NGINX_EXTERNAL_PORT".to_string(), NGINX_EXTERNAL_PORT.to_string());
    env.insert("SOCKET".to_string(), socket_path.to_string_lossy().to_string());

    // Suppress unused warning — paths is used by callers for socket_path resolution
    let _ = paths;

    Ok(())
}

/// Write nginx-related variables to the app's ENV file if not already present.
fn persist_nginx_env(
    app: &str,
    kind: &str,
    _command: &str,
    socket_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    let env_dir = paths.env_root.join(app);
    fs::create_dir_all(&env_dir)?;
    let env_file = env_dir.join("ENV");

    let mut content = if env_file.exists() {
        fs::read_to_string(&env_file)?
    } else {
        String::new()
    };

    // Only append once — skip if already written by a previous deploy.
    if content.contains("NGINX_PORTMAP") || content.contains("NGINX_WSGI") {
        return Ok(());
    }

    if is_wsgi_kind(kind) {
        content.push_str("NGINX_WSGI=true\n");
        content.push_str(&format!("UWSGI_SOCKET={}\n", socket_path.display()));
    } else {
        let port = env.get("PORT").map(|s| s.as_str()).unwrap_or("8080");
        content.push_str("NGINX_PORTMAP=true\n");
        content.push_str(&format!("NGINX_INTERNAL_PORT={}\n", port));
        content.push_str(&format!("NGINX_EXTERNAL_PORT={}\n", NGINX_EXTERNAL_PORT));
    }

    fs::write(&env_file, &content)?;
    Ok(())
}

/// Write a single worker TOML config to `workers_available` and symlink it into `workers_enabled`.
fn write_worker_config(
    app: &str,
    app_path: &Path,
    kind: &str,
    command: &str,
    index: u32,
    env: HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::supervisor::config::create_worker_config;

    let log_path = paths
        .log_root
        .join(app)
        .join(format!("{}.{}.log", kind, index));

    let config = create_worker_config(
        app,
        kind,
        command,
        index,
        env,
        &app_path.to_string_lossy(),
        &log_path.to_string_lossy(),
    );

    let filename = format!("{}-{}-{}.toml", app, kind, index);
    let available = paths.workers_available.join(&filename);
    let enabled = paths.workers_enabled.join(&filename);

    fs::write(&available, toml::to_string(&config)?)?;

    if enabled.exists() {
        fs::remove_file(&enabled)?;
    }
    std::os::unix::fs::symlink(&available, &enabled)?;

    echo(&format!("-----> Created worker config: {}", filename), "green");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        let paths = crate::config::RikuPaths::from_dirs(
            tmp.path().join(".riku"),
            &tmp.path().to_path_buf(),
        );
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.nginx_root).unwrap();
        paths
    }

    // --- read_scaling_count ---

    #[test]
    fn test_read_scaling_count_default_when_no_file() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        let count = read_scaling_count(&paths, "myapp", "web")?;
        assert_eq!(count, 1, "Default scaling count should be 1");
        Ok(())
    }

    #[test]
    fn test_read_scaling_count_reads_file() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=3\nworker=2\n")?;
        assert_eq!(read_scaling_count(&paths, "myapp", "web")?, 3);
        assert_eq!(read_scaling_count(&paths, "myapp", "worker")?, 2);
        Ok(())
    }

    #[test]
    fn test_read_scaling_count_unknown_kind_returns_one() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "web=2\n")?;
        assert_eq!(read_scaling_count(&paths, "myapp", "cron")?, 1);
        Ok(())
    }

    #[test]
    fn test_read_scaling_count_ignores_comments() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("SCALING"), "# web=99\nweb=1\n")?;
        assert_eq!(read_scaling_count(&paths, "myapp", "web")?, 1);
        Ok(())
    }

    // --- create_workers_generic ---

    #[test]
    fn test_create_workers_generic_no_procfile_returns_ok() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let env = HashMap::new();
        create_workers_generic("myapp", &app_path, &env, &paths, None)
    }

    #[test]
    fn test_create_workers_generic_worker_kind_creates_config() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        fs::write(app_path.join("Procfile"), "worker: python worker.py\n")?;

        let env = HashMap::new();
        create_workers_generic("myapp", &app_path, &env, &paths, None)?;

        let config_path = paths.workers_available.join("myapp-worker-1.toml");
        assert!(config_path.exists(), "worker config should be created");
        Ok(())
    }

    #[test]
    fn test_create_workers_generic_symlink_created_in_enabled() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        fs::write(app_path.join("Procfile"), "worker: python worker.py\n")?;

        let env = HashMap::new();
        create_workers_generic("myapp", &app_path, &env, &paths, None)?;

        let symlink_path = paths.workers_enabled.join("myapp-worker-1.toml");
        assert!(symlink_path.exists(), "symlink in workers_enabled should exist");
        Ok(())
    }

    #[test]
    fn test_create_workers_generic_skips_comment_lines() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        fs::write(app_path.join("Procfile"), "# comment\nworker: echo hello\n")?;

        let env = HashMap::new();
        create_workers_generic("myapp", &app_path, &env, &paths, None)?;

        let entries: Vec<_> = fs::read_dir(&paths.workers_available)
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(entries.len(), 1);
        Ok(())
    }

    #[test]
    fn test_auto_restart_false_skips_removal_of_existing_configs() -> anyhow::Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app_path = tmp.path().join("app");
        fs::create_dir_all(&app_path).unwrap();
        fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
        fs::create_dir_all(paths.log_root.join("myapp")).unwrap();

        let existing = paths.workers_enabled.join("myapp-web-1.toml");
        fs::write(&existing, "[worker]\n")?;

        fs::write(app_path.join("Procfile"), "worker: echo hello\n")?;

        let mut env = HashMap::new();
        env.insert("RIKU_AUTO_RESTART".to_string(), "false".to_string());
        create_workers_generic("myapp", &app_path, &env, &paths, None)?;

        assert!(existing.exists(), "existing config should be preserved when RIKU_AUTO_RESTART=false");
        Ok(())
    }
}

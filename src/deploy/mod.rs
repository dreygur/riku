//! Deployment orchestration module.
//!
//! # Security Model
//!
//! **Anyone with git push access to a riku server can execute arbitrary commands
//! on the host.** Procfile commands (web, worker, preflight, release) are run via
//! `sh -c` as the riku user with no sandboxing. This is inherent to the PaaS model.
//!
//! Operators MUST:
//! - Only grant SSH access to trusted users
//! - Run riku under a dedicated unprivileged user account
//! - Consider additional isolation (containers, namespaces) for untrusted workloads
//!
//! Input validation is applied to app names, environment variables, and plugin
//! names to prevent path traversal and injection, but deployed application code
//! itself runs with full user-level privileges.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use which::which;

use crate::config::RikuPaths;
use crate::util::{echo, found_app, parse_procfile};

pub mod clojure;
pub mod container;
pub mod container_runtime;
pub mod go;
pub mod identity;
pub mod java;
pub mod node;
pub mod python;
pub mod ruby;
pub mod rust;

/// Supported application runtimes, detected from marker files.
#[derive(Debug, PartialEq)]
pub enum Runtime {
    Python,
    PythonPoetry,
    PythonUv,
    Node,
    Ruby,
    Go,
    Rust,
    JavaMaven,
    JavaGradle,
    ClojureCli,
    ClojureLein,
    Container,
    #[allow(dead_code)]
    Identity,
}

impl std::fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runtime::Python => write!(f, "Python"),
            Runtime::PythonPoetry => write!(f, "Python (Poetry)"),
            Runtime::PythonUv => write!(f, "Python (uv)"),
            Runtime::Node => write!(f, "Node"),
            Runtime::Ruby => write!(f, "Ruby"),
            Runtime::Go => write!(f, "Go"),
            Runtime::Rust => write!(f, "Rust"),
            Runtime::JavaMaven => write!(f, "Java Maven"),
            Runtime::JavaGradle => write!(f, "Java Gradle"),
            Runtime::ClojureCli => write!(f, "Clojure CLI"),
            Runtime::ClojureLein => write!(f, "Clojure Lein"),
            Runtime::Container => write!(f, "Container"),
            Runtime::Identity => write!(f, "Identity"),
        }
    }
}

/// Detect the application runtime by checking marker files in the app directory.
pub fn detect_runtime(app_path: &Path) -> Option<Runtime> {
    // 1. requirements.txt -> Python
    if app_path.join("requirements.txt").exists() {
        return Some(Runtime::Python);
    }

    // 2-4. pyproject.toml with poetry/uv/fallback
    if app_path.join("pyproject.toml").exists() {
        if which("poetry").is_ok() {
            return Some(Runtime::PythonPoetry);
        }
        if which("uv").is_ok() {
            return Some(Runtime::PythonUv);
        }
        // fallback: plain Python
        return Some(Runtime::Python);
    }

    // 5. Gemfile -> Ruby
    if app_path.join("Gemfile").exists() {
        return Some(Runtime::Ruby);
    }

    // 6. package.json -> Node
    if app_path.join("package.json").exists() {
        return Some(Runtime::Node);
    }

    // 7. pom.xml -> JavaMaven
    if app_path.join("pom.xml").exists() {
        return Some(Runtime::JavaMaven);
    }

    // 8. build.gradle -> JavaGradle
    if app_path.join("build.gradle").exists() {
        return Some(Runtime::JavaGradle);
    }

    // 9. Godeps or go.mod or *.go files -> Go
    if app_path.join("Godeps").exists() || app_path.join("go.mod").exists() {
        return Some(Runtime::Go);
    }
    if let Ok(entries) = fs::read_dir(app_path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "go" {
                    return Some(Runtime::Go);
                }
            }
        }
    }

    // 10. deps.edn -> ClojureCli
    if app_path.join("deps.edn").exists() {
        return Some(Runtime::ClojureCli);
    }

    // 11. project.clj -> ClojureLein
    if app_path.join("project.clj").exists() {
        return Some(Runtime::ClojureLein);
    }

    // 12. Dockerfile or Containerfile -> Container
    if app_path.join("Dockerfile").exists() || app_path.join("Containerfile").exists() {
        return Some(Runtime::Container);
    }

    // 13. docker-compose.yml or podman-compose.yml -> Container
    if app_path.join("docker-compose.yml").exists()
        || app_path.join("docker-compose.yaml").exists()
        || app_path.join("podman-compose.yml").exists()
        || app_path.join("podman-compose.yaml").exists()
        || app_path.join("compose.yml").exists()
        || app_path.join("compose.yaml").exists()
    {
        return Some(Runtime::Container);
    }

    // 16. Cargo.toml + rust-toolchain.toml -> Rust
    if app_path.join("Cargo.toml").exists() && app_path.join("rust-toolchain.toml").exists() {
        return Some(Runtime::Rust);
    }

    // 17. No runtime detected
    None
}

/// Deploy an app by resetting the work directory, detecting runtime, and spawning workers.
pub fn do_deploy(
    app: &str,
    paths: &RikuPaths,
    _deltas: &HashMap<String, i64>,
    newrev: Option<&str>,
) -> Result<()> {
    let app_path = paths.app_root.join(app);
    let log_path = paths.log_root.join(app);

    if !app_path.exists() {
        echo(&format!("Error: app '{}' not found.", app), "red");
        return Ok(());
    }

    echo(&format!("-----> Deploying app '{}'", app), "green");

    // Git fetch and reset if newrev provided
    // Fetch from origin to get the latest changes
    // Clear GIT_DIR and GIT_WORK_TREE that might be set by git hooks
    let git_fetch_result = Command::new("git")
        .args(["fetch", "--quiet", "origin"])
        .current_dir(&app_path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .status();

    if let Err(e) = git_fetch_result {
        echo(&format!("Warning: git fetch failed: {}", e), "yellow");
    }

    if let Some(rev) = newrev {
        let git_reset_result = Command::new("git")
            .args(["reset", "--hard", rev])
            .current_dir(&app_path)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .status();

        if let Err(e) = git_reset_result {
            echo(&format!("Warning: git reset failed: {}", e), "yellow");
        }
    }

    // Ensure log directory exists
    if !log_path.exists() {
        fs::create_dir_all(&log_path)?;
    }

    // Parse Procfile
    let procfile = app_path.join("Procfile");
    let workers = parse_procfile(&procfile);

    let mut workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            echo(
                &format!("Error: Invalid Procfile for app '{}'.", app),
                "red",
            );
            return Ok(());
        }
    };

    // Run preflight command if present
    if let Some(preflight_cmd) = workers.remove("preflight") {
        echo("-----> Running preflight.", "green");
        let status = Command::new("sh")
            .arg("-c")
            .arg(&preflight_cmd)
            .current_dir(&app_path)
            .status()?;
        if !status.success() {
            let code = status.code().unwrap_or(1);
            echo(
                &format!(
                    "-----> Exiting due to preflight command error value: {}",
                    code
                ),
                "",
            );
            std::process::exit(code);
        }
    }

    // Get environment variables for the app
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    // Validate environment variables and print warnings
    let warnings = crate::util::validate_env_vars(&env);
    crate::util::print_env_warnings(&warnings);

    // Detect and deploy runtime
    let runtime = detect_runtime(&app_path);
    match &runtime {
        Some(rt) => {
            found_app(&rt.to_string());

            // Call the appropriate deployer
            match rt {
                Runtime::Python => {
                    python::deploy_python(app, &app_path, &env, paths)?;
                }
                Runtime::PythonPoetry => {
                    python::deploy_python_poetry(app, &app_path, &env, paths)?;
                }
                Runtime::PythonUv => {
                    python::deploy_python_uv(app, &app_path, &env, paths)?;
                }
                Runtime::Node => {
                    node::deploy_node(app, &app_path, &env, paths)?;
                }
                Runtime::Ruby => {
                    ruby::deploy_ruby(app, &app_path, &env, paths)?;
                }
                Runtime::Go => {
                    go::deploy_go(app, &app_path, &env, paths)?;
                }
                Runtime::JavaMaven => {
                    java::deploy_java_maven(app, &app_path, &env, paths)?;
                }
                Runtime::JavaGradle => {
                    java::deploy_java_gradle(app, &app_path, &env, paths)?;
                }
                Runtime::ClojureCli => {
                    clojure::deploy_clojure_cli(app, &app_path, &env, paths)?;
                }
                Runtime::ClojureLein => {
                    clojure::deploy_clojure_lein(app, &app_path, &env, paths)?;
                }
                Runtime::Container => {
                    // Deploy using the available container runtime
                    crate::deploy::container::deploy_container(app, &app_path, &env, paths)?;
                }
                Runtime::Rust => {
                    rust::deploy_rust(app, &app_path, &env, paths)?;
                }
                Runtime::Identity => {
                    // Identity deployment for generic apps
                    identity::deploy_identity(app, &app_path, &env, paths)?;
                }
            }
        }
        None => {
            // Check for identity-style deployments (PHP, release+web, static)
            if workers.contains_key("release") && workers.contains_key("web") {
                echo("-----> Generic app detected.", "green");
                // For now, treat as identity deployment
                identity::create_identity_workers(app, &app_path, &env, paths)?;
            } else if workers.contains_key("static") {
                echo("-----> Static app detected.", "green");
                identity::create_identity_workers(app, &app_path, &env, paths)?;
            } else {
                echo("-----> Could not detect runtime!", "red");
            }
        }
    }

    // Run release command if present
    if let Some(release_cmd) = workers.remove("release") {
        echo("-----> Releasing", "green");
        let output = Command::new("sh")
            .arg("-c")
            .arg(&release_cmd)
            .current_dir(&app_path)
            .output()?;
        if !output.status.success() {
            let code = output.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            echo(
                &format!(
                    "Error: Release command failed with exit code {}: {}",
                    code, stderr
                ),
                "red",
            );
            std::process::exit(code);
        } else {
            // Optionally log stdout on success
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                echo(&format!("Release output: {}", stdout.trim()), "green");
            }
        }
    }

    // Call spawn_app to start the application processes
    spawn_app(app, paths)?;

    Ok(())
}

/// Create worker configurations for identity-style deployments.
#[allow(dead_code)]
fn create_identity_workers(
    app: &str,
    app_path: &Path,
    workers: &HashMap<String, String>,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    use crate::supervisor::config::create_worker_config;
    use crate::util::get_free_port;
    use std::collections::HashMap;
    use std::fs;

    // Handle PIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("PIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
        // Remove existing worker configs to trigger restart
        for ext in &["toml", "ini"] {
            let pattern = paths.workers_enabled.join(format!("{}*.{}", app, ext));
            if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(&entry);
                }
            }
        }
    }

    for (kind, command) in workers {
        if kind == "release" || kind == "preflight" {
            continue; // Skip release and preflight commands as they're run once during deploy
        }

        // Parse scaling info if available
        let scaling_path = paths.env_root.join(app).join("SCALING");
        let mut count = 1; // default to 1 instance

        if scaling_path.exists() {
            let scaling_content = fs::read_to_string(&scaling_path)?;
            for scale_line in scaling_content.lines() {
                let scale_line = scale_line.trim();
                if scale_line.is_empty() || scale_line.starts_with('#') {
                    continue;
                }

                if let Some(scale_pos) = scale_line.find('=') {
                    let scale_kind = scale_line[..scale_pos].trim();
                    let scale_count_str = scale_line[scale_pos + 1..].trim();

                    if scale_kind == kind {
                        if let Ok(scale_count) = scale_count_str.parse::<u32>() {
                            count = scale_count;
                        }
                        break;
                    }
                }
            }
        }

        // Check for RIKU_WORKER_PROCESSES env var (format: "web=2,worker=1")
        let env_file = paths.env_root.join(app).join("ENV");
        let mut app_env: HashMap<String, String> = HashMap::new();
        if env_file.exists() {
            crate::util::parse_settings(&env_file, &mut app_env)?;
        }

        if let Some(worker_processes) = app_env.get("RIKU_WORKER_PROCESSES") {
            for proc_def in worker_processes.split(',') {
                let proc_def = proc_def.trim();
                if let Some(eq_pos) = proc_def.find('=') {
                    let proc_kind = proc_def[..eq_pos].trim();
                    let proc_count_str = proc_def[eq_pos + 1..].trim();

                    if proc_kind == kind {
                        if let Ok(proc_count) = proc_count_str.parse::<u32>() {
                            count = proc_count;
                        }
                        break;
                    }
                }
            }
        }

        // Create worker configs for each instance
        for i in 1..=count {
            // Prepare environment for the worker
            let env_file = paths.env_root.join(app).join("ENV");
            let mut worker_env: HashMap<String, String> = HashMap::new();
            if env_file.exists() {
                crate::util::parse_settings(&env_file, &mut worker_env)?;
            }

            // Set PORT for web processes
            let final_command = command.clone();
            if kind == "web" {
                let port = get_free_port("127.0.0.1")?;
                worker_env.insert("PORT".to_string(), port.to_string());

                // Create socket file for web processes
                let socket_path = paths.nginx_root.join(format!("{}.sock", app));
                worker_env.insert(
                    "SOCKET".to_string(),
                    socket_path.to_string_lossy().to_string(),
                );
            }

            // Create the worker config
            let worker_config = create_worker_config(
                app,
                kind,
                &final_command,
                i,
                worker_env,
                &app_path.to_string_lossy(),
                &paths
                    .log_root
                    .join(app)
                    .join(format!("{}.{}.log", kind, i))
                    .to_string_lossy(),
            );

            // Write the worker config to the available directory
            let config_filename = format!("{}-{}-{}.toml", app, kind, i);
            let config_path = paths.workers_available.join(&config_filename);

            let config_content = toml::to_string(&worker_config)?;
            fs::write(&config_path, &config_content)?;

            // Create a symlink to enable the worker
            let enabled_path = paths.workers_enabled.join(&config_filename);
            if enabled_path.exists() {
                fs::remove_file(&enabled_path)?;
            }
            std::os::unix::fs::symlink(&config_path, &enabled_path)?;

            crate::util::echo(
                &format!("-----> Created worker config: {}", config_filename),
                "green",
            );
        }
    }

    Ok(())
}

/// Notify the supervisor to reload configurations (if running).
fn notify_supervisor_reload() {
    // Send SIGHUP to the supervisor process to trigger config reload
    if let Ok(output) = std::process::Command::new("pgrep")
        .args(["-f", "riku supervisor"])
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.split_whitespace() {
                if let Ok(pid_num) = pid.parse::<i32>() {
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid_num),
                        nix::sys::signal::Signal::SIGHUP,
                    );
                }
            }
        }
    }
}

/// Notify the supervisor to reload configurations and spawn processes.
/// This function is called after deployment to start/restart application processes.
/// The worker configs should already exist from the deploy step.
pub fn spawn_app(app: &str, paths: &RikuPaths) -> Result<()> {
    use crate::util::echo;
    use std::collections::HashMap;

    let app_path = paths.app_root.join(app);

    // Get environment variables for nginx config generation
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    // Generate nginx configuration
    let nginx_result = crate::nginx::generate_nginx_config(app, &app_path, &env, paths);
    if let Err(e) = nginx_result {
        echo(
            &format!("Warning: Failed to generate nginx config: {}", e),
            "yellow",
        );
    }

    // Notify the supervisor to reload configurations
    // The supervisor will detect new/changed configs and spawn processes
    notify_supervisor_reload();

    echo("-----> Notified supervisor to spawn processes...", "green");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_marker(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    #[test]
    fn test_detect_python_requirements() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "requirements.txt");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Python));
    }

    #[test]
    fn test_detect_pyproject_fallback_to_python() {
        // If neither poetry nor uv are on PATH, pyproject.toml falls back to Python.
        // This test may detect Poetry or uv if installed; we just verify it returns Some.
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "pyproject.toml");
        let rt = detect_runtime(tmp.path());
        assert!(rt.is_some());
        // It should be one of the Python variants
        match rt.unwrap() {
            Runtime::Python | Runtime::PythonPoetry | Runtime::PythonUv => {}
            other => panic!("Expected a Python variant, got {:?}", other),
        }
    }

    #[test]
    fn test_detect_ruby() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Gemfile");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Ruby));
    }

    #[test]
    fn test_detect_node() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "package.json");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Node));
    }

    #[test]
    fn test_detect_java_maven() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "pom.xml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::JavaMaven));
    }

    #[test]
    fn test_detect_java_gradle() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "build.gradle");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::JavaGradle));
    }

    #[test]
    fn test_detect_go_godeps() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Godeps");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_go_mod() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "go.mod");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_go_files() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "main.go");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_clojure_cli() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "deps.edn");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::ClojureCli));
    }

    #[test]
    fn test_detect_clojure_lein() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "project.clj");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::ClojureLein));
    }

    #[test]
    fn test_detect_rust() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Cargo.toml");
        create_marker(tmp.path(), "rust-toolchain.toml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Rust));
    }

    #[test]
    fn test_detect_rust_needs_both_files() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Cargo.toml");
        // Without rust-toolchain.toml, should not detect Rust
        assert_eq!(detect_runtime(tmp.path()), None);
    }

    #[test]
    fn test_detect_no_runtime() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_runtime(tmp.path()), None);
    }

    #[test]
    fn test_priority_requirements_over_pyproject() {
        // requirements.txt takes precedence over pyproject.toml
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "requirements.txt");
        create_marker(tmp.path(), "pyproject.toml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Python));
    }

    #[test]
    fn test_priority_gemfile_over_package_json() {
        // Gemfile appears before package.json in detection order
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Gemfile");
        create_marker(tmp.path(), "package.json");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Ruby));
    }

    #[test]
    fn test_runtime_display() {
        assert_eq!(Runtime::Python.to_string(), "Python");
        assert_eq!(Runtime::PythonPoetry.to_string(), "Python (Poetry)");
        assert_eq!(Runtime::PythonUv.to_string(), "Python (uv)");
        assert_eq!(Runtime::Node.to_string(), "Node");
        assert_eq!(Runtime::Go.to_string(), "Go");
        assert_eq!(Runtime::Rust.to_string(), "Rust");
    }
}

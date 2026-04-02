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

use crate::config::RikuPaths;
use crate::util::{echo, found_app, parse_procfile};

pub mod clojure;
pub mod container;
pub mod container_export;
pub(self) mod container_workers;
pub mod container_runtime;
pub mod env_setup;
pub mod git_ops;
pub mod go;
pub mod hooks;
pub mod identity;
pub mod java;
pub mod node;
pub(self) mod node_workers;
pub mod python;
pub(self) mod python_workers;
pub mod ruby;
pub mod rust;
pub mod runtime;
pub mod scaling;
pub mod supervisor_ctl;
pub mod workers;

// macros.rs only defines macros — Rust exports them via #[macro_export]
// so they appear at crate root automatically; no `pub mod macros` needed.
#[allow(clippy::module_inception)]
mod macros;

pub use runtime::{detect_runtime, Runtime};
pub use supervisor_ctl::spawn_app;
pub use workers::{create_workers_generic, read_scaling_count};

/// Deploy an app by resetting the work directory, detecting runtime, and spawning workers.
pub fn do_deploy(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    newrev: Option<&str>,
) -> Result<()> {
    let app_path = paths.app_root.join(app);
    let log_path = paths.log_root.join(app);

    if !app_path.exists() {
        return Err(anyhow::anyhow!(
            "App '{}' not found at {}",
            app,
            app_path.display()
        ));
    }

    echo(&format!("-----> Deploying app '{}'", app), "green");

    // Sync working tree with the pushed revision.
    git_ops::sync_app_repo(&app_path, newrev)?;

    // Ensure log directory exists.
    if !log_path.exists() {
        fs::create_dir_all(&log_path)?;
    }

    // Parse Procfile.
    let workers = parse_procfile(&app_path.join("Procfile"));
    let mut workers = match workers {
        Some(w) if !w.is_empty() => w,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid or missing Procfile for app '{}'. Deploy aborted.",
                app
            ));
        }
    };

    // Apply scaling deltas if any.
    let _scaling_counts = workers::apply_scaling_deltas(app, paths, deltas, &workers)?;

    // Run preflight command if present.
    if let Some(preflight_cmd) = workers.remove("preflight") {
        env_setup::run_preflight(&preflight_cmd, &app_path);
    }

    // Load app environment variables.
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env: HashMap<String, String> = HashMap::new();
    if env_file.exists() {
        crate::util::parse_settings(&env_file, &mut env)?;
    }

    // Validate environment variables and print warnings.
    let warnings = crate::util::validate_env_vars(&env);
    crate::util::print_env_warnings(&warnings);

    // pre-deploy hook (failures abort the deploy).
    hooks::run_pre_deploy(app, &app_path, paths, &env)?;

    // Detect runtime (needed for hook context and dispatch).
    let runtime = detect_runtime(&app_path);
    let runtime_name = runtime.as_ref().map(|r| r.to_string());

    // pre-build hook (failures abort the deploy).
    hooks::run_pre_build(app, &app_path, paths, runtime_name.as_deref(), &env)?;

    // Dispatch to runtime-specific deployer.
    dispatch_runtime(app, &app_path, &mut env, paths, &runtime, &workers)?;

    // post-build hook (failures abort the deploy).
    hooks::run_post_build(app, &app_path, paths, runtime_name.as_deref(), &env)?;

    // Run release command if present.
    if let Some(release_cmd) = workers.get("release") {
        env_setup::run_release(release_cmd, &app_path)?;
    }

    // Write LIVE_ENV with resolved environment.
    env_setup::write_live_env(app, paths, &env)?;

    // Start the application processes.
    spawn_app(app, paths)?;

    // post-deploy hook (failures are warnings, not fatal).
    let _ = hooks::run_post_deploy(app, &app_path, paths, runtime_name.as_deref(), &env);

    Ok(())
}

/// Dispatch deployment to the appropriate runtime-specific handler.
fn dispatch_runtime(
    app: &str,
    app_path: &std::path::Path,
    env: &mut HashMap<String, String>,
    paths: &RikuPaths,
    runtime: &Option<Runtime>,
    workers: &HashMap<String, String>,
) -> Result<()> {
    match runtime {
        Some(rt) => {
            found_app(&rt.to_string());
            match rt {
                Runtime::Python => python::deploy_python(app, app_path, env, paths)?,
                Runtime::PythonPoetry => python::deploy_python_poetry(app, app_path, env, paths)?,
                Runtime::PythonUv => python::deploy_python_uv(app, app_path, env, paths)?,
                Runtime::Node => node::deploy_node(app, app_path, env, paths)?,
                Runtime::Ruby => ruby::deploy_ruby(app, app_path, env, paths)?,
                Runtime::Go => go::deploy_go(app, app_path, env, paths)?,
                Runtime::JavaMaven => java::deploy_java_maven(app, app_path, env, paths)?,
                Runtime::JavaGradle => java::deploy_java_gradle(app, app_path, env, paths)?,
                Runtime::ClojureCli => clojure::deploy_clojure_cli(app, app_path, env, paths)?,
                Runtime::ClojureLein => clojure::deploy_clojure_lein(app, app_path, env, paths)?,
                Runtime::Container => container::deploy_container(app, app_path, env, paths)?,
                Runtime::Rust => rust::deploy_rust(app, app_path, env, paths)?,
                Runtime::Wsgi | Runtime::Jwsgi | Runtime::Rwsgi | Runtime::Php => {
                    env_setup::setup_wsgi_env(app, paths, env)?;
                    identity::deploy_identity(app, app_path, env, paths)?;
                }
                Runtime::Identity => identity::deploy_identity(app, app_path, env, paths)?,
            }
        }
        None => {
            if workers.contains_key("release") && workers.contains_key("web") {
                echo("-----> Generic app detected.", "green");
                found_app(&Runtime::Identity.to_string());
                identity::create_identity_workers(app, app_path, env, paths)?;
            } else if workers.contains_key("static") {
                echo("-----> Static app detected.", "green");
                found_app(&Runtime::Identity.to_string());
                identity::create_identity_workers(app, app_path, env, paths)?;
            } else {
                echo("-----> Could not detect runtime!", "red");
            }
        }
    }
    Ok(())
}

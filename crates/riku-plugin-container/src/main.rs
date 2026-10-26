//! Riku runtime plugin: Container
//!
//! Handles apps with a `Dockerfile`, `Containerfile`, or a compose file
//! (`compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`,
//! or the one `COMPOSE_FILE` names). Auto-detects Docker or Podman.
//! Subcommands: detect, build, env, start, pull-service.
//!
//! Compose apps pull their registry images, then build every service that
//! has a `build:` section. If `RIKU_GHCR_USERNAME`/`RIKU_GHCR_TOKEN` are set in the
//! app's ENV file, the plugin logs in to `ghcr.io` before pulling.

mod compose_cli;
mod compose_file;

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use compose_cli::{compose_cmd, compose_cmd_display, pull_and_build, run_compose};
use compose_file::{compose_file, default_compose_file, requested_compose_file};

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let arg2 = std::env::args().nth(2);
    let app_path = std::env::var("RIKU_APP_PATH").unwrap_or_else(|_| ".".into());
    let app = std::env::var("RIKU_APP").unwrap_or_else(|_| "app".into());
    let env_path = std::env::var("RIKU_ENV_PATH").ok().map(PathBuf::from);
    let app_path = Path::new(&app_path);

    match cmd.as_str() {
        "detect" => detect(app_path),
        "build" => build(app_path, &app, env_path.as_deref()),
        "env" => print_env(app_path, &app),
        "start" => print_start(app_path, &app),
        "pull-service" => {
            let service = arg2.ok_or_else(|| anyhow!("pull-service requires a service name"))?;
            pull_service(app_path, &service, env_path.as_deref())
        }
        other => bail!("Unknown subcommand: {}", other),
    }
}

/// Whether this plugin should handle the app.
///
/// A set `COMPOSE_FILE` claims the app even if it names a bad path, so the
/// validation error surfaces from `build` instead of another runtime plugin
/// silently taking the deploy.
fn handles_app(app_path: &Path, requested: Option<&str>) -> bool {
    requested.is_some()
        || app_path.join("Dockerfile").exists()
        || app_path.join("Containerfile").exists()
        || default_compose_file(app_path).is_some()
}

fn detect(app_path: &Path) -> Result<()> {
    if handles_app(app_path, requested_compose_file().as_deref()) {
        std::process::exit(0);
    }
    std::process::exit(1);
}

fn runtime() -> &'static str {
    if which("podman") {
        "podman"
    } else {
        "docker"
    }
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Read `KEY=VALUE` lines from the app's ENV file (ignoring comments/blanks).
/// Returns an empty map if there is no env path or no file yet.
fn read_env_file(env_path: Option<&Path>) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    let Some(env_path) = env_path else {
        return vars;
    };
    let Ok(content) = fs::read_to_string(env_path.join("ENV")) else {
        return vars;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            vars.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    vars
}

/// Log in to GHCR if `RIKU_GHCR_USERNAME`/`RIKU_GHCR_TOKEN` are configured. No-op otherwise.
fn ghcr_login(env_path: Option<&Path>) -> Result<()> {
    let vars = read_env_file(env_path);
    let (Some(user), Some(token)) = (vars.get("RIKU_GHCR_USERNAME"), vars.get("RIKU_GHCR_TOKEN"))
    else {
        return Ok(());
    };

    let mut child = Command::new(runtime())
        .args(["login", "ghcr.io", "-u", user, "--password-stdin"])
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("child spawned with piped stdin")
        .write_all(token.as_bytes())?;
    if !child.wait()?.success() {
        bail!("GHCR login failed");
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn build(app_path: &Path, app: &str, env_path: Option<&Path>) -> Result<()> {
    let compose = match compose_file(app_path) {
        Ok(compose) => compose,
        Err(e) => return Err(e),
    };
    if let Some(compose) = compose {
        if let Err(e) = ghcr_login(env_path) {
            return Err(e);
        }
        return pull_and_build(app_path, &compose);
    }

    let rt = runtime();
    let image = format!("riku-{}", app);

    let dockerfile = if app_path.join("Containerfile").exists() {
        "Containerfile"
    } else {
        "Dockerfile"
    };

    let status = match Command::new(rt)
        .args(["build", "-t", &image, "-f", dockerfile, "."])
        .current_dir(app_path)
        .status()
    {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("failed to run {rt} build: {e}")),
    };

    if !status.success() {
        bail!("Container build failed");
    }
    Ok(())
}

#[allow(clippy::question_mark)]
fn print_env(app_path: &Path, app: &str) -> Result<()> {
    let compose = match compose_file(app_path) {
        Ok(compose) => compose,
        Err(e) => return Err(e),
    };
    if let Some(compose) = compose {
        println!("CONTAINER_RUNTIME={}", runtime());
        println!("COMPOSE_FILE={}", compose.arg());
        return Ok(());
    }
    let image = format!("riku-{}", app);
    println!("CONTAINER_IMAGE={}", image);
    println!("CONTAINER_RUNTIME={}", runtime());
    Ok(())
}

#[allow(clippy::question_mark)]
fn print_start(app_path: &Path, app: &str) -> Result<()> {
    let compose = match compose_file(app_path) {
        Ok(compose) => compose,
        Err(e) => return Err(e),
    };
    if let Some(compose) = compose {
        let cmd = compose_cmd();
        println!("{} -f {} up", compose_cmd_display(&cmd), compose.arg());
        return Ok(());
    }

    let rt = runtime();
    let image = format!("riku-{}", app);
    println!("{} run --rm -p $PORT:$PORT {}", rt, image);
    Ok(())
}

/// Pull and recreate a single compose service, without touching the rest of
/// the stack. Used by the supervisor's periodic image-watch check to apply
/// a freshly pushed image.
#[allow(clippy::question_mark)]
fn pull_service(app_path: &Path, service: &str, env_path: Option<&Path>) -> Result<()> {
    let compose = match compose_file(app_path) {
        Ok(Some(compose)) => compose,
        Ok(None) => bail!("No compose file found in {}", app_path.display()),
        Err(e) => return Err(e),
    };
    if let Err(e) = ghcr_login(env_path) {
        return Err(e);
    }

    let pull_step = format!("compose pull for service '{service}'");
    if let Err(e) = run_compose(app_path, &compose, &["pull", service], &pull_step) {
        return Err(e);
    }

    let up_step = format!("compose up for service '{service}'");
    run_compose(
        app_path,
        &compose,
        &["up", "-d", "--no-deps", service],
        &up_step,
    )
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

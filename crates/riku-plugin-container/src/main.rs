//! Riku runtime plugin: Container
//!
//! Handles apps with a `Dockerfile`, `Containerfile`, or `docker-compose.yml`.
//! Auto-detects Docker or Podman. Supports subcommands: detect, build, env, start.

use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let app_path = std::env::var("RIKU_APP_PATH").unwrap_or_else(|_| ".".into());
    let app = std::env::var("RIKU_APP").unwrap_or_else(|_| "app".into());
    let app_path = Path::new(&app_path);

    match cmd.as_str() {
        "detect" => detect(app_path),
        "build" => build(app_path, &app),
        "env" => print_env(app_path, &app),
        "start" => print_start(app_path, &app),
        other => bail!("Unknown subcommand: {}", other),
    }
}

fn detect(app_path: &Path) -> Result<()> {
    if app_path.join("Dockerfile").exists()
        || app_path.join("Containerfile").exists()
        || app_path.join("docker-compose.yml").exists()
        || app_path.join("compose.yml").exists()
    {
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

fn build(app_path: &Path, app: &str) -> Result<()> {
    let rt = runtime();
    let image = format!("riku-{}", app);

    let dockerfile = if app_path.join("Containerfile").exists() {
        "Containerfile"
    } else {
        "Dockerfile"
    };

    let status = Command::new(rt)
        .args(["build", "-t", &image, "-f", dockerfile, "."])
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        bail!("Container build failed");
    }
    Ok(())
}

fn print_env(_app_path: &Path, app: &str) -> Result<()> {
    let image = format!("riku-{}", app);
    println!("CONTAINER_IMAGE={}", image);
    println!("CONTAINER_RUNTIME={}", runtime());
    Ok(())
}

fn print_start(_app_path: &Path, app: &str) -> Result<()> {
    let rt = runtime();
    let image = format!("riku-{}", app);
    println!("{} run --rm -p $PORT:$PORT {}", rt, image);
    Ok(())
}

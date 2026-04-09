//! Riku runtime plugin: Java
//!
//! Handles Maven (`pom.xml`) and Gradle (`build.gradle`) apps.
//! Supports subcommands: detect, build, env, start.

use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let app_path = std::env::var("RIKU_APP_PATH").unwrap_or_else(|_| ".".into());
    let app_path = Path::new(&app_path);

    match cmd.as_str() {
        "detect" => detect(app_path),
        "build" => build(app_path),
        "env" => print_env(app_path),
        "start" => print_start(app_path),
        other => bail!("Unknown subcommand: {}", other),
    }
}

fn detect(app_path: &Path) -> Result<()> {
    if app_path.join("pom.xml").exists() || app_path.join("build.gradle").exists() {
        std::process::exit(0);
    }
    std::process::exit(1);
}

fn build(app_path: &Path) -> Result<()> {
    if app_path.join("pom.xml").exists() {
        run_build(app_path, "mvn", &["package", "-DskipTests", "--batch-mode"])
    } else if app_path.join("build.gradle").exists() {
        let gradle = if app_path.join("gradlew").exists() {
            "./gradlew"
        } else {
            "gradle"
        };
        run_build(app_path, gradle, &["build", "-x", "test"])
    } else {
        bail!("No pom.xml or build.gradle found");
    }
}

fn run_build(app_path: &Path, bin: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(bin)
        .args(args)
        .current_dir(app_path)
        .status()?;
    if !status.success() {
        bail!("Java build failed with exit code {:?}", status.code());
    }
    Ok(())
}

fn print_env(_app_path: &Path) -> Result<()> {
    println!("JAVA_OPTS=-Xmx512m");
    Ok(())
}

fn print_start(app_path: &Path) -> Result<()> {
    if app_path.join("pom.xml").exists() {
        // Try to find the jar in target/
        println!("java $JAVA_OPTS -jar target/*.jar");
    } else {
        println!("java $JAVA_OPTS -jar build/libs/*.jar");
    }
    Ok(())
}

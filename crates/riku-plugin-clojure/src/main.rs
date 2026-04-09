//! Riku runtime plugin: Clojure
//!
//! Handles Leiningen (`project.clj`) and Clojure CLI (`deps.edn`) apps.
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
    if app_path.join("project.clj").exists() || app_path.join("deps.edn").exists() {
        std::process::exit(0);
    }
    std::process::exit(1);
}

fn build(app_path: &Path) -> Result<()> {
    if app_path.join("project.clj").exists() {
        let status = Command::new("lein")
            .args(["uberjar"])
            .current_dir(app_path)
            .status()?;
        if !status.success() {
            bail!("lein uberjar failed");
        }
    } else if app_path.join("deps.edn").exists() {
        // Deps.edn — run clojure -T:build uber if build.clj exists, else no-op
        if app_path.join("build.clj").exists() {
            let status = Command::new("clojure")
                .args(["-T:build", "uber"])
                .current_dir(app_path)
                .status()?;
            if !status.success() {
                bail!("clojure build failed");
            }
        }
    }
    Ok(())
}

fn print_env(_app_path: &Path) -> Result<()> {
    println!("JVM_OPTS=-Xmx512m");
    Ok(())
}

fn print_start(app_path: &Path) -> Result<()> {
    if app_path.join("project.clj").exists() {
        println!("java $JVM_OPTS -jar target/uberjar/*.jar");
    } else {
        println!("clojure -M -m core");
    }
    Ok(())
}

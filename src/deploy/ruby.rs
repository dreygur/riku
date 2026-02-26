//! Ruby application deployment module.
//!
//! Handles deployment of Ruby applications using Bundler.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::deploy::create_workers_generic;
use crate::util::echo;

/// Deploy a Ruby application using Bundler.
pub fn deploy_ruby(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(&format!("-----> Deploying Ruby app '{}'", app), "green");

    // Create isolated gem directory in ENV_ROOT
    let bundle_path = paths.env_root.join(app).join("vendor");
    fs::create_dir_all(&bundle_path)?;

    // Configure bundle to use isolated path
    echo("-----> Configuring bundle with isolated gem path", "green");
    let status = Command::new("bundle")
        .args([
            "config",
            "set",
            "--local",
            "path",
            &bundle_path.to_string_lossy(),
        ])
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        echo(
            "-----> Failed to configure bundle path, using default",
            "yellow",
        );
    }

    // Install dependencies with Bundler
    echo("-----> Installing dependencies with Bundler", "green");
    let status = Command::new("bundle")
        .arg("install")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to install dependencies with Bundler"
        ));
    }

    // Add Ruby-specific environment variables
    let mut ruby_env = env.clone();
    ruby_env.insert("RACK_ENV".to_string(), "production".to_string());
    ruby_env.insert("RAILS_ENV".to_string(), "production".to_string());
    ruby_env.insert(
        "BUNDLE_PATH".to_string(),
        bundle_path.to_string_lossy().to_string(),
    );

    // Create worker configurations (generic implementation)
    create_workers_generic(app, app_path, &ruby_env, paths)?;

    Ok(())
}

//! Java application deployment module.
//!
//! Handles deployment of Java applications using Maven or Gradle.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::deploy::read_scaling_count;
use crate::setup_web_port;
use crate::util::echo;
use crate::write_worker_config;

/// Deploy a Java application using Maven.
pub fn deploy_java_maven(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(
        &format!("-----> Deploying Java (Maven) app '{}'", app),
        "green",
    );

    // Build the Java application with Maven
    echo("-----> Building Java application with Maven", "green");
    let status = Command::new("mvn")
        .arg("clean")
        .arg("package")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to build Java application with Maven"
        ));
    }

    // Create worker configurations
    create_java_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Deploy a Java application using Gradle.
pub fn deploy_java_gradle(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    echo(
        &format!("-----> Deploying Java (Gradle) app '{}'", app),
        "green",
    );

    // Build the Java application with Gradle
    echo("-----> Building Java application with Gradle", "green");
    let status = Command::new("gradle")
        .arg("build")
        .current_dir(app_path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to build Java application with Gradle"
        ));
    }

    // Create worker configurations
    create_java_workers(app, app_path, env, paths)?;

    Ok(())
}

/// Create worker configurations for Java applications.
fn create_java_workers(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    // Handle RIKU_AUTO_RESTART - if false, skip removing existing worker configs
    let auto_restart = env
        .get("RIKU_AUTO_RESTART")
        .map(|v| v.to_lowercase() != "false" && v != "0" && v != "no")
        .unwrap_or(true);

    if auto_restart {
        for ext in &["toml", "ini"] {
            let pattern = paths.workers_enabled.join(format!("{}-*.{}", app, ext));
            if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(&entry);
                }
            }
        }
    }

    // Read Procfile to determine processes to run
    let procfile_path = app_path.join("Procfile");
    if !procfile_path.exists() {
        echo(
            "-----> No Procfile found, skipping process creation",
            "yellow",
        );
        return Ok(());
    }

    let procfile_content = fs::read_to_string(&procfile_path)?;
    for line in procfile_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(pos) = line.find(':') {
            let kind = line[..pos].trim();
            let command = line[pos + 1..].trim();

            let count = read_scaling_count(paths, app, kind)?;

            for i in 1..=count {
                create_java_worker_config(app, kind, command, i, env, paths, app_path)?;
            }
        }
    }

    Ok(())
}

/// Create a single worker configuration for a Java process.
fn create_java_worker_config(
    app: &str,
    kind: &str,
    command: &str,
    ordinal: u32,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    app_path: &Path,
) -> Result<()> {
    let mut worker_env = env.clone();

    // Add Java-specific environment variables
    worker_env.insert("JAVA_OPTS".to_string(), "-Xmx512m -Xms256m".to_string());

    // Set PORT for web processes and determine final command
    let final_command = if kind == "web" {
        let port = setup_web_port!(worker_env, app, paths);

        // Inject -Dserver.port= for Spring-Boot-style jars that don't already pin a port
        if command.contains("-Dserver.port=") || command.contains("--port=") {
            command.to_string()
        } else if command.contains("java")
            && (command.contains("-jar") || command.contains("spring-boot"))
        {
            // Insert -Dserver.port= before the -jar argument
            let jar_part = command.split("-jar").nth(1).unwrap_or("").trim();
            let pre_jar = command.split("-jar").next().unwrap_or("").trim();
            format!("{} -Dserver.port={} -jar {}", pre_jar, port, jar_part)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    write_worker_config!(
        app,
        kind,
        &final_command,
        ordinal,
        worker_env,
        app_path,
        paths
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_create_java_worker_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = crate::config::RikuPaths::from_dirs(
            temp_dir.path().join(".piku"),
            &temp_dir.path().to_path_buf(),
        );

        // Create necessary directories
        fs::create_dir_all(&paths.workers_available).unwrap();
        fs::create_dir_all(&paths.workers_enabled).unwrap();
        fs::create_dir_all(&paths.log_root.join("testapp")).unwrap();

        let mut env = HashMap::new();
        env.insert("ENV_VAR".to_string(), "value".to_string());

        let result = create_java_worker_config(
            "testapp",
            "web",
            "java -jar app.jar",
            1,
            &env,
            &paths,
            temp_dir.path(),
        );

        assert!(result.is_ok());

        // Check that the config file was created
        let config_path = paths.workers_available.join("testapp-web-1.toml");
        assert!(config_path.exists());
    }
}

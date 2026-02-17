//! Java application deployment module.
//!
//! Handles deployment of Java applications using Maven or Gradle.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::supervisor::config::create_worker_config;
use crate::util::{echo, get_free_port};

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

            // Create worker configs for each instance
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
    // Prepare environment for the worker
    let mut worker_env = env.clone();

    // Set PORT for web processes
    if kind == "web" {
        let port = get_free_port("127.0.0.1").expect("Failed to find a free port for web process");
        worker_env.insert("PORT".to_string(), port.to_string());

        // Update command to include port if it's a web process
        let _updated_command = if command.contains("-Dserver.port=") || command.contains("--port=")
        {
            command.to_string()
        } else {
            // If it's a common Java web server, add port binding
            if command.contains("java")
                && (command.contains("-jar") || command.contains("spring-boot"))
            {
                format!(
                    "{} -Dserver.port={} -jar {}",
                    command.split_whitespace().collect::<Vec<_>>()[1..].join(" "),
                    port,
                    command.split("-jar").nth(1).unwrap_or("").trim()
                )
            } else {
                command.to_string()
            }
        };

        // Create socket file for web processes
        let socket_path = paths.nginx_root.join(format!("{}.sock", app));
        worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );
    }

    // Add Java-specific environment variables
    worker_env.insert("JAVA_OPTS".to_string(), "-Xmx512m -Xms256m".to_string());

    // Create the worker config
    let worker_config = create_worker_config(
        app,
        kind,
        command, // Using the original command parameter
        ordinal,
        worker_env,
        &app_path.to_string_lossy(),
        &paths
            .log_root
            .join(app)
            .join(format!("{}.{}.log", kind, ordinal))
            .to_string_lossy(),
    );

    // Write the worker config to the available directory
    let config_filename = format!("{}-{}-{}.toml", app, kind, ordinal);
    let config_path = paths.workers_available.join(&config_filename);

    let config_content = toml::to_string(&worker_config)?;
    fs::write(&config_path, config_content)?;

    // Create a symlink to enable the worker
    let enabled_path = paths.workers_enabled.join(&config_filename);
    if enabled_path.exists() {
        fs::remove_file(&enabled_path)?;
    }
    std::os::unix::fs::symlink(&config_path, &enabled_path)?;

    echo(
        &format!("-----> Created worker config: {}", config_filename),
        "green",
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

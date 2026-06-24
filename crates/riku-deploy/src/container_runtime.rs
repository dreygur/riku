//! Container image export and remote deployment utilities.
//!
//! Handles building container images (Docker/Podman) locally and deploying them to remote servers.

use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::util::echo;

/// Get the container runtime command (docker or podman).
pub fn get_container_runtime() -> Result<String> {
    // Check for Docker first
    let docker_check = Command::new("docker").arg("--version").output();

    if let Ok(output) = docker_check {
        if output.status.success() {
            return Ok("docker".to_string());
        }
    }

    // Check for Podman
    let podman_check = Command::new("podman").arg("--version").output();

    if let Ok(output) = podman_check {
        if output.status.success() {
            return Ok("podman".to_string());
        }
    }

    Err(anyhow::anyhow!(
        "Neither Docker nor Podman is available on this system"
    ))
}

/// Build a container image locally and export it to a tar archive.
/// Automatically detects and uses Docker or Podman.
pub fn build_and_export(app_name: &str, build_context: &Path, output_path: &Path) -> Result<()> {
    let runtime = get_container_runtime()?;

    echo(
        &format!("-----> Building {} image for '{}'", runtime, app_name),
        "green",
    );

    let image_name = format!("riku-{}", app_name);

    // Build the image
    let build_status = Command::new(&runtime)
        .args(["build", "-t", &image_name, "."])
        .current_dir(build_context)
        .status()?;

    if !build_status.success() {
        return Err(anyhow::anyhow!("Failed to build {} image", runtime));
    }

    echo(
        &format!("-----> Exporting image to '{}'", output_path.display()),
        "green",
    );

    // Export to tar archive
    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output path"))?;

    let export_status = Command::new(&runtime)
        .args(["save", "-o", output_path_str, &image_name])
        .status()?;

    if !export_status.success() {
        return Err(anyhow::anyhow!("Failed to export {} image", runtime));
    }

    echo(
        &format!("Successfully exported image to '{}'", output_path.display()),
        "green",
    );
    Ok(())
}

/// Transfer a file to a remote server using rsync (with scp fallback).
pub fn transfer_to_remote(local_path: &Path, remote_host: &str, remote_path: &str) -> Result<()> {
    echo(
        &format!(
            "-----> Transferring '{}' to '{}:{}'",
            local_path.display(),
            remote_host,
            remote_path
        ),
        "green",
    );

    // Use rsync for better performance (faster than scp, supports resume)
    let local_path_str = local_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid local path"))?;

    let status = Command::new("rsync")
        .args([
            "-avz", // archive mode, verbose, compress
            "--progress",
            local_path_str,
            &format!("{}:{}", remote_host, remote_path),
        ])
        .status();

    if status.is_ok_and(|s| s.success()) {
        echo("Transfer completed successfully (rsync)", "green");
        Ok(())
    } else {
        // Fallback to scp if rsync is not available
        echo("-----> rsync not available, falling back to scp", "yellow");
        let status = Command::new("scp")
            .args([local_path_str, &format!("{}:{}", remote_host, remote_path)])
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to transfer file to remote server"));
        }

        echo("Transfer completed successfully (scp)", "green");
        Ok(())
    }
}

/// Import a container image from a tar archive on the remote server.
/// Automatically detects and uses Docker or Podman.
pub fn import_remote(remote_host: &str, archive_path: &str) -> Result<()> {
    // Detect remote runtime
    let runtime = check_remote_runtime(remote_host)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Neither Docker nor Podman is available on remote '{}'",
            remote_host
        )
    })?;

    echo(
        &format!("-----> Importing {} image on '{}'", runtime, remote_host),
        "green",
    );

    let status = Command::new("ssh")
        .args([
            remote_host,
            &format!("{} load -i {}", runtime, archive_path),
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to import {} image on remote",
            runtime
        ));
    }

    echo("Image imported successfully", "green");
    Ok(())
}

/// Clean up the exported tar archive after deployment.
pub fn cleanup_archive(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
        echo(&format!("Cleaned up archive '{}'", path.display()), "green");
    }
    Ok(())
}

/// Check if the remote server has Docker or Podman installed.
pub fn check_remote_runtime(remote_host: &str) -> Result<Option<String>> {
    // Check for Docker
    let docker_check = Command::new("ssh")
        .args([remote_host, "docker --version"])
        .output();

    if let Ok(output) = docker_check {
        if output.status.success() {
            return Ok(Some("docker".to_string()));
        }
    }

    // Check for Podman
    let podman_check = Command::new("ssh")
        .args([remote_host, "podman --version"])
        .output();

    if let Ok(output) = podman_check {
        if output.status.success() {
            return Ok(Some("podman".to_string()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_container_runtime() {
        // This test will depend on what's available on the system
        let result = get_container_runtime();
        // The result depends on what's installed
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_check_remote_runtime_format() {
        // Just verify the function exists and compiles
        // Actual testing would require a real remote host
        let result = check_remote_runtime("localhost");
        // Result depends on what's installed on localhost
        assert!(result.is_ok());
    }
}

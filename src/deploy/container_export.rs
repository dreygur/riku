//! Container image export/import deployment utilities.
//!
//! Provides alternative deployment paths for pre-built or locally-built
//! container images. These are supplementary to the main [`super::container`]
//! deploy flow.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::util::echo;

use super::container::detect_container_runtime;
use super::container_workers::{create_container_workers, create_container_workers_from_image};

/// Deploy a pre-built container image from a tar archive.
#[allow(dead_code)]
pub fn deploy_container_export(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    image_tar_path: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Loading container image for app '{}' with {}",
                    app, runtime
                ),
                "green",
            );

            let status = Command::new(&runtime)
                .args(["load", "-i", image_tar_path])
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to load {} image from archive",
                    runtime
                ));
            }

            create_container_workers(app, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Deploy a locally built container image to remote.
#[allow(dead_code)]
pub fn deploy_local_image(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    image_name: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Deploying local image '{}' for app '{}' with {}",
                    image_name, app, runtime
                ),
                "green",
            );

            let tagged_image = format!("riku-{}", app);
            let status = Command::new(&runtime)
                .args(["tag", image_name, &tagged_image])
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to tag {} image", runtime));
            }

            create_container_workers_from_image(app, &tagged_image, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

/// Build container image locally and deploy it.
#[allow(dead_code)]
pub fn build_and_deploy_locally(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    build_context: &str,
) -> Result<()> {
    match detect_container_runtime()? {
        Some(runtime) => {
            echo(
                &format!(
                    "-----> Building and deploying app '{}' locally with {}",
                    app, runtime
                ),
                "green",
            );

            let image_name = format!("riku-{}", app);
            let status = Command::new(&runtime)
                .args(["build", "-t", &image_name, build_context])
                .current_dir(app_path)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Failed to build {} image", runtime));
            }

            create_container_workers_from_image(app, &image_name, app_path, env, paths, &runtime)
        }
        None => Err(anyhow::anyhow!(
            "Neither Docker nor Podman is available on this system"
        )),
    }
}

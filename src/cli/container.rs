//! CLI commands for container image export and remote deployment.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::RikuPaths;
use crate::deploy::container_runtime;
use crate::util::echo;

/// Container export and remote deployment commands.
#[derive(Parser, Debug)]
pub struct ContainerCmd {
    #[command(subcommand)]
    pub command: ContainerSubCmd,
}

#[derive(Subcommand, Debug)]
pub enum ContainerSubCmd {
    /// Build container image locally and export to tar archive (auto-detects Docker/Podman)
    Export {
        /// Application name
        #[arg(short, long)]
        app: String,

        /// Build context path (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        context: PathBuf,

        /// Output path for the tar archive
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Build locally, export, transfer to remote, and deploy (auto-detects Docker/Podman)
    DeployRemote {
        /// Application name
        #[arg(short, long)]
        app: String,

        /// Build context path (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        context: PathBuf,

        /// Remote host (user@host)
        #[arg(short, long)]
        remote: String,

        /// Remote path to store the archive temporarily
        #[arg(long, default_value = "/tmp")]
        remote_tmp_path: String,

        /// Keep the local archive after deployment
        #[arg(long)]
        keep_archive: bool,
    },

    /// Deploy an exported tar archive to remote (auto-detects Docker/Podman)
    DeployArchive {
        /// Application name
        #[arg(short, long)]
        app: String,

        /// Path to the tar archive
        #[arg(short, long)]
        archive: PathBuf,

        /// Remote host (user@host)
        #[arg(short, long)]
        remote: String,
    },

    /// Check if remote has Docker or Podman installed
    CheckRemote {
        /// Remote host (user@host)
        #[arg(short, long)]
        remote: String,
    },
}

/// Execute container export command.
pub fn cmd_container(cmd: ContainerCmd, _paths: &RikuPaths) -> Result<()> {
    match cmd.command {
        ContainerSubCmd::Export {
            app,
            context,
            output,
        } => {
            container_runtime::build_and_export(&app, &context, &output)?;
        }

        ContainerSubCmd::DeployRemote {
            app,
            context,
            remote,
            remote_tmp_path,
            keep_archive,
        } => {
            // Step 1: Build and export locally
            let archive_name = format!("riku-{}.tar", app);
            let archive_path = std::env::current_dir()?.join(&archive_name);

            container_runtime::build_and_export(&app, &context, &archive_path)?;

            // Step 2: Transfer to remote
            let remote_archive_path = format!("{}/{}", remote_tmp_path, archive_name);
            container_runtime::transfer_to_remote(&archive_path, &remote, &remote_archive_path)?;

            // Step 3: Import on remote (auto-detects Docker/Podman)
            container_runtime::import_remote(&remote, &remote_archive_path)?;

            // Step 4: Clean up remote archive
            let cleanup_status = std::process::Command::new("ssh")
                .args([&remote, &format!("rm -f {}", remote_archive_path)])
                .status()?;

            if cleanup_status.success() {
                echo("Cleaned up remote archive", "green");
            }

            // Step 5: Clean up local archive if not keeping
            if !keep_archive {
                container_runtime::cleanup_archive(&archive_path)?;
            }

            echo(&format!("Successfully deployed '{}' to '{}'", app, remote), "green");
        }

        ContainerSubCmd::DeployArchive {
            app,
            archive,
            remote,
        } => {
            // Step 1: Transfer to remote
            let archive_name = archive.file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid archive path"))?
                .to_string_lossy();
            let remote_archive_path = format!("/tmp/{}", archive_name);
            container_runtime::transfer_to_remote(&archive, &remote, &remote_archive_path)?;

            // Step 2: Import on remote (auto-detects Docker/Podman)
            container_runtime::import_remote(&remote, &remote_archive_path)?;

            // Step 3: Clean up remote archive
            let cleanup_status = std::process::Command::new("ssh")
                .args([&remote, &format!("rm -f {}", remote_archive_path)])
                .status()?;

            if cleanup_status.success() {
                echo("Cleaned up remote archive", "green");
            }

            echo(&format!("Successfully deployed '{}' to '{}'", app, remote), "green");
        }

        ContainerSubCmd::CheckRemote { remote } => {
            let runtime = container_runtime::check_remote_runtime(&remote)?;
            match runtime {
                Some(rt) => {
                    echo(&format!("Remote '{}' has {} installed", remote, rt), "green");
                }
                None => {
                    echo(&format!("Remote '{}' has neither Docker nor Podman installed", remote), "red");
                }
            }
        }
    }

    Ok(())
}
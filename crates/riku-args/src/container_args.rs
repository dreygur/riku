//! Argument definitions for the `container` subcommands.

use clap::Subcommand;
use std::path::PathBuf;

/// Subcommands of `riku container`.
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
        #[arg(long)]
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

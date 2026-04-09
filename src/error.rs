//! Crate-level error types.
//!
//! Typed errors for the three core domains: deployment, plugins, and nginx.
//! Utility and setup code may still use `anyhow` for one-off failures.

use std::path::PathBuf;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Deploy errors
// ---------------------------------------------------------------------------

/// Errors that can occur during the application deployment pipeline.
#[derive(Debug, Error)]
pub enum DeployError {
    /// The requested app directory does not exist.
    #[error("app '{0}' not found")]
    AppNotFound(String),

    /// Procfile is missing, empty, or contains no valid entries.
    #[error("invalid or missing Procfile for app '{0}'")]
    ProcfileInvalid(String),

    /// A runtime build step failed (npm, pip, cargo, etc.).
    #[error("{runtime} build failed for app '{app}': {reason}")]
    BuildFailed {
        runtime: &'static str,
        app: String,
        reason: String,
    },

    /// A git operation (fetch, reset) failed.
    #[error("git sync failed for app '{app}': {reason}")]
    GitSyncFailed { app: String, reason: String },

    /// Worker config file could not be written.
    #[error("failed to write worker config '{path}': {reason}")]
    WorkerConfigFailed { path: PathBuf, reason: String },
}

// ---------------------------------------------------------------------------
// Plugin errors
// ---------------------------------------------------------------------------

/// Errors from the plugin discovery and hook-execution system.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin directory does not exist or cannot be read.
    #[error("plugin directory not accessible: {0}")]
    DirectoryUnreadable(String),

    /// A plugin with the given name was not found.
    #[error("plugin '{0}' not found")]
    NotFound(String),

    /// Plugin name contains disallowed characters.
    #[error("invalid plugin name '{0}'")]
    InvalidName(String),

    /// Plugin process could not be spawned.
    #[error("failed to spawn plugin '{name}': {reason}")]
    SpawnFailed { name: String, reason: String },

    /// A blocking hook (pre-deploy, pre-build) exited with failure.
    #[error("hook '{hook}' failed for app '{app}': {reason}")]
    HookFailed {
        hook: &'static str,
        app: String,
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Nginx errors
// ---------------------------------------------------------------------------

/// Errors from nginx configuration generation and validation.
#[derive(Debug, Error)]
pub enum NginxError {
    /// Template rendering failed.
    #[error("failed to render nginx template for app '{0}': {1}")]
    TemplateFailed(String, String),

    /// `nginx -t` validation rejected the generated config.
    #[error("nginx config validation failed for app '{0}': {1}")]
    ValidationFailed(String, String),

    /// An SSL certificate operation failed.
    #[error("SSL setup failed for app '{0}': {1}")]
    SslFailed(String, String),

    /// A domain name contains disallowed characters.
    #[error("invalid server name '{0}'")]
    InvalidServerName(String),
}

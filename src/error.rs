//! Crate-level error types.
//!
//! Typed errors for the three core domains: deployment, plugins, and nginx.
//! Utility and setup code may still use `anyhow` for one-off failures.

#![allow(dead_code)]

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

    /// Another deploy for this app is already running. Distinct from other
    /// `DeployError` variants so callers (e.g. the dashboard's control API)
    /// can map it to a 409 Conflict instead of a 500.
    #[error("deploy already in progress for app '{0}'")]
    DeployInProgress(String),

    /// A build/preflight/release step was terminated by an enforced
    /// resource limit (cgroup `memory.max`, `RLIMIT_AS`/`RLIMIT_CPU`, or the
    /// kernel OOM killer) rather than failing on its own. Carries the fully
    /// formatted, human-readable diagnostic block (built by
    /// [`DeployError::resource_exhausted`]) so the CLI can print it
    /// directly instead of a bare "exited with code N".
    #[error("{0}")]
    ResourceExhausted(String),
}

impl DeployError {
    /// Build a [`DeployError::ResourceExhausted`] with a structured,
    /// actionable diagnostic: what limit was hit, why the step was halted,
    /// and what to do about it.
    ///
    /// `stage` is one of `"build"`, `"preflight"`, `"release"`; `command` is
    /// the plugin name or shell command that was running; `cause` is the
    /// specific classification from
    /// [`crate::plugins::executor::classify_resource_exit`] (e.g. "killed
    /// by SIGKILL — the kernel's OOM killer...").
    pub fn resource_exhausted(stage: &str, command: &str, cause: &str) -> Self {
        DeployError::ResourceExhausted(format!(
            "riku deploy halted — resource limit exceeded\n\
             \x20 stage   : {stage}\n\
             \x20 command : {command}\n\
             \x20 cause   : {cause}\n\
             \x20 impact  : the {stage} step was halted to protect the host — this is not a\n\
             \x20           bug in your application code, it ran past an enforced resource\n\
             \x20           ceiling\n\
             \x20 remedy  : raise the relevant limit for this deploy — RIKU_MAX_MEMORY_MB\n\
             \x20           (default 512) for memory, RIKU_MAX_CPU_SECONDS (default 3600) for\n\
             \x20           CPU time — or reduce the {stage} step's own memory/CPU footprint",
            stage = stage,
            command = command,
            cause = cause,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_exhausted_includes_all_diagnostic_fields() {
        let err = DeployError::resource_exhausted(
            "build",
            "node",
            "killed by SIGKILL — the kernel's OOM killer terminated it directly",
        );
        let message = err.to_string();

        for field in ["stage", "command", "cause", "impact", "remedy"] {
            assert!(
                message.contains(field),
                "diagnostic missing '{}' field:\n{}",
                field,
                message
            );
        }
        assert!(message.contains("build"));
        assert!(message.contains("node"));
        assert!(message.contains("SIGKILL"));
        assert!(message.contains("RIKU_MAX_MEMORY_MB"));
        assert!(message.contains("RIKU_MAX_CPU_SECONDS"));
    }

    #[test]
    fn test_resource_exhausted_is_distinct_variant() {
        let err = DeployError::resource_exhausted("release", "make migrate", "some cause");
        assert!(matches!(err, DeployError::ResourceExhausted(_)));
    }
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

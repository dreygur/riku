//! Sub-command enums for the Riku CLI.

use clap::Subcommand;

/// Apps subcommands
#[derive(Subcommand, Debug)]
pub enum AppsCmd {
    /// Create a new application
    #[command(after_help = "Examples:\n  riku apps create myapp")]
    Create {
        /// Application name
        name: String,
    },

    /// Show detailed information about an application
    #[command(after_help = "Examples:\n  riku apps info myapp")]
    Info {
        /// Application name
        name: String,
    },

    /// Destroy an application (preserves data/cache)
    Destroy {
        /// Application name
        name: String,
    },
}

/// Config subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Show app configuration
    #[command(after_help = "Examples:\n  riku config show myapp")]
    Show {
        /// App name
        app: String,
    },

    /// Get a single configuration value
    #[command(after_help = "Examples:\n  riku config get myapp DATABASE_URL")]
    Get {
        /// App name
        app: String,
        /// Configuration key
        key: String,
    },

    /// Set environment variables (triggers redeploy)
    #[command(
        trailing_var_arg = true,
        after_help = "Examples:\n  riku config set myapp DATABASE_URL=postgres://localhost/myapp\n  riku config set myapp KEY1=val1 KEY2=val2"
    )]
    Set {
        /// App name
        app: String,
        /// KEY=VAL pairs
        #[arg(required = true)]
        settings: Vec<String>,
    },

    /// Remove environment variables (triggers redeploy)
    #[command(
        trailing_var_arg = true,
        after_help = "Examples:\n  riku config unset myapp OLD_KEY"
    )]
    Unset {
        /// App name
        app: String,
        /// Keys to remove
        #[arg(required = true)]
        keys: Vec<String>,
    },

    /// Show live running configuration
    #[command(after_help = "Examples:\n  riku config live myapp")]
    Live {
        /// App name
        app: String,
    },
}

/// Stats subcommands
#[derive(Subcommand, Debug)]
pub enum StatsCmd {
    /// Show stats for all apps
    All,

    /// Show stats for a specific app
    App {
        /// App name
        app: String,
    },
}

/// Client-side plugin management commands
#[derive(Subcommand, Debug)]
pub enum PluginCmd {
    /// List installed client plugins
    List,

    /// Check if a client plugin exists and is executable
    Exists {
        /// Plugin name
        name: String,
    },
}

/// Server-side lifecycle hook plugin management commands
#[derive(Subcommand, Debug)]
pub enum HookCmd {
    /// List all executable server-side hook plugins (~/.riku/plugins/)
    List,

    /// Check if a server-side hook plugin exists and is executable
    #[command(
        after_help = "Examples:\n  riku hook check riku-pre-deploy\n  riku hook check riku-post-deploy"
    )]
    Check {
        /// Hook plugin name (e.g. riku-pre-deploy)
        name: String,
    },
}

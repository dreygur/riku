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
    #[command(after_help = "Examples:\n  riku apps destroy myapp")]
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
    #[command(after_help = "Examples:\n  riku stats all")]
    All,

    /// Show stats for a specific app
    #[command(after_help = "Examples:\n  riku stats app myapp")]
    App {
        /// App name
        app: String,
    },
}

/// Client-side plugin management commands
#[derive(Subcommand, Debug)]
pub enum PluginCmd {
    /// List installed client plugins
    #[command(after_help = "Examples:\n  riku plugin list")]
    List,

    /// Check if a client plugin exists and is executable
    #[command(after_help = "Examples:\n  riku plugin exists riku-deploy")]
    Exists {
        /// Plugin name
        name: String,
    },
}

/// Server-side lifecycle hook plugin management commands
#[derive(Subcommand, Debug)]
pub enum HookCmd {
    /// List all executable server-side hook plugins (~/.riku/plugins/)
    #[command(after_help = "Examples:\n  riku hook list")]
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

/// Addon (managed resource) commands — Plugin Protocol v1 addon seam.
#[derive(Subcommand, Debug)]
pub enum AddonCmd {
    /// List provisioned addon instances
    #[command(after_help = "Examples:\n  riku addon list")]
    List,

    /// Provision a new instance of an addon plugin
    #[command(after_help = "Examples:\n  riku addon create postgres db1")]
    Create {
        /// Addon plugin name (e.g. postgres)
        plugin: String,
        /// Instance name (unique)
        name: String,
    },

    /// Bind an instance to an app, injecting its env
    #[command(after_help = "Examples:\n  riku addon bind db1 myapp")]
    Bind {
        /// Instance name
        instance: String,
        /// App name
        app: String,
    },

    /// Unbind an instance from an app, removing its env
    #[command(after_help = "Examples:\n  riku addon unbind db1 myapp")]
    Unbind {
        /// Instance name
        instance: String,
        /// App name
        app: String,
    },

    /// Destroy an instance (must be unbound first)
    #[command(after_help = "Examples:\n  riku addon destroy db1")]
    Destroy {
        /// Instance name
        instance: String,
    },

    /// Back up an instance
    #[command(after_help = "Examples:\n  riku addon backup db1")]
    Backup {
        /// Instance name
        instance: String,
    },
}

/// Plugin bundle management (manifest-based bundles, ROADMAP E2).
#[derive(Subcommand, Debug)]
pub enum PluginsCmd {
    /// Install a plugin bundle from a local path or git URL
    #[command(
        after_help = "Examples:\n  riku plugins install ./examples/plugins/postgres\n  riku plugins install github:riku-plugins/redis"
    )]
    Install {
        /// Local directory or git URL (github:owner/repo, https://…/repo.git)
        source: String,
    },

    /// List installed plugin bundles
    #[command(after_help = "Examples:\n  riku plugins list")]
    List,

    /// Remove an installed plugin bundle
    #[command(after_help = "Examples:\n  riku plugins remove postgres")]
    Remove {
        /// Plugin name
        name: String,
    },

    /// Search registered marketplaces
    #[command(after_help = "Examples:\n  riku plugins search postgres")]
    Search {
        /// Query (matches name and description; empty lists all)
        #[arg(default_value = "")]
        query: String,
    },

    /// Install a plugin by name from a registered marketplace
    #[command(
        after_help = "Examples:\n  riku plugins add postgres\n  riku plugins add postgres@official"
    )]
    Add {
        /// Plugin spec: name or name@marketplace
        spec: String,
    },

    /// Validate installed plugin bundles (API compatibility + integrity)
    #[command(after_help = "Examples:\n  riku plugins doctor")]
    Doctor,

    /// Manage marketplaces (git repos that index plugins)
    #[command(subcommand)]
    Marketplace(MarketplaceCmd),
}

/// Marketplace registration commands.
#[derive(Subcommand, Debug)]
pub enum MarketplaceCmd {
    /// Register and clone a marketplace
    #[command(
        after_help = "Examples:\n  riku plugins marketplace add github:dreygur/riku-marketplace"
    )]
    Add {
        /// Git URL (github:owner/repo, https://…/repo.git)
        url: String,
        /// Override the derived marketplace name
        #[arg(long)]
        name: Option<String>,
    },

    /// List registered marketplaces
    List,

    /// Remove a registered marketplace
    Remove {
        /// Marketplace name
        name: String,
    },
}

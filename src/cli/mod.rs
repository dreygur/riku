use clap::{Parser, Subcommand};

pub mod apps;
pub mod client_plugins;
pub mod container;
pub mod git;
pub mod scp;
pub mod setup;

/// riku — the smallest PaaS you've ever seen (Rust edition)
#[derive(Parser, Debug)]
#[command(name = "riku", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List deployed apps
    Apps,

    /// Manage app configuration
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Container export and remote deployment commands
    Container {
        #[command(subcommand)]
        cmd: container::ContainerSubCmd,
    },

    /// Force redeploy of an app
    Deploy {
        /// App name
        app: String,
    },

    /// Remove an app (preserves data dir)
    Destroy {
        /// App name
        app: String,
    },

    /// Tail app logs
    Logs {
        /// App name
        app: String,
        /// Process filter (default: all)
        #[arg(default_value = "*")]
        process: String,
    },

    /// Manage app processes
    #[command(subcommand)]
    Ps(PsCmd),

    /// Show process stats and metrics
    #[command(subcommand)]
    Stats(StatsCmd),

    /// Run a command in the app context
    #[command(trailing_var_arg = true)]
    Run {
        /// App name
        app: String,
        /// Command and arguments to run
        #[arg(required = true)]
        cmd: Vec<String>,
    },

    /// Restart an app (hot reload for zero downtime)
    Restart {
        /// App name
        app: String,
        /// Use hot reload (zero downtime)
        #[arg(long, short)]
        hot: bool,
    },

    /// Stop an app
    Stop {
        /// App name
        app: String,
    },

    /// Initialize Riku server (directories + systemd + SSH)
    Init {
        /// Skip systemd service setup
        #[arg(long)]
        no_systemd: bool,
    },

    /// Self-update the riku binary
    Update,

    /// Start the process supervisor daemon
    Supervisor,

    /// List or manage client plugins
    #[command(subcommand)]
    Plugin(PluginCmd),

    /// Git post-receive hook (internal)
    #[command(hide = true)]
    GitHook {
        /// App name
        app: String,
    },

    /// Git receive-pack (internal)
    #[command(hide = true)]
    GitReceivePack {
        /// App name
        app: String,
    },

    /// Git upload-pack (internal)
    #[command(hide = true)]
    GitUploadPack {
        /// App name
        app: String,
    },

    /// SCP handler (internal)
    #[command(
        hide = true,
        trailing_var_arg = true,
        allow_external_subcommands = true
    )]
    Scp {
        /// SCP arguments
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Show app configuration
    Show {
        /// App name
        app: String,
    },

    /// Get a single configuration value
    Get {
        /// App name
        app: String,
        /// Configuration key
        key: String,
    },

    /// Set environment variables (triggers redeploy)
    #[command(trailing_var_arg = true)]
    Set {
        /// App name
        app: String,
        /// KEY=VAL pairs
        #[arg(required = true)]
        settings: Vec<String>,
    },

    /// Remove environment variables (triggers redeploy)
    #[command(trailing_var_arg = true)]
    Unset {
        /// App name
        app: String,
        /// Keys to remove
        #[arg(required = true)]
        keys: Vec<String>,
    },

    /// Show live running configuration
    Live {
        /// App name
        app: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PsCmd {
    /// Show process status (detailed view)
    Show {
        /// App name
        app: String,
        /// Show detailed info including health status
        #[arg(long, short)]
        verbose: bool,
    },

    /// Scale workers
    #[command(trailing_var_arg = true)]
    Scale {
        /// App name
        app: String,
        /// Worker scaling settings (e.g. web=4 worker=2)
        #[arg(required = true)]
        settings: Vec<String>,
    },
}

/// Stats commands
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

/// Plugin management commands
#[derive(Subcommand, Debug)]
pub enum PluginCmd {
    /// List available client plugins
    List,

    /// Check if a plugin exists
    Exists {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SetupCmd {
    /// Initialize ~/.piku directory structure
    Init,

    /// Add an SSH public key
    Ssh {
        /// Path to public key file
        pubkey: String,
    },
}

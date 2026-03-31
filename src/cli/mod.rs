use clap::{Parser, Subcommand};

pub mod agent;
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
    Apps {
        #[command(subcommand)]
        cmd: Option<AppsCmd>,
    },

    /// AI agent interface (SSH-based automation)
    Agent {
        /// Show agent introduction and permissions
        #[arg(long)]
        intro: bool,

        /// Show full command schema (JSON)
        #[arg(long)]
        schema: bool,

        /// Show help for a command
        #[arg(long, name = "agent-help")]
        agent_help: bool,

        /// Command to execute
        #[arg()]
        command: Option<String>,

        /// Arguments for the command
        #[arg(last = true)]
        args: Vec<String>,

        /// Confirmation token for destructive actions
        #[arg(long)]
        confirm: Option<String>,

        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

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

        /// Deploy from local path instead of git repo
        #[arg(long)]
        from: Option<String>,
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
    Ps {
        /// Show all processes (default) or specify an app
        #[arg()]
        app: Option<String>,
        /// Show detailed info including health status
        #[arg(long, short)]
        verbose: bool,
        /// Scale workers (e.g. web=4 worker=2)
        #[arg(short, long, num_args = 1..)]
        scale: Vec<String>,
    },

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
        /// Actual repo path (optional)
        repo_path: Option<String>,
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

    /// Check if a client plugin exists
    Exists {
        /// Plugin name
        name: String,
    },

    /// List all executable server-side hook plugins (~/.riku/plugins/)
    Hooks,

    /// Check if a server-side hook plugin exists and is executable
    Check {
        /// Hook plugin name (e.g. riku-post-deploy)
        name: String,
    },
}

/// Apps subcommands
#[derive(Subcommand, Debug)]
pub enum AppsCmd {
    /// Create a new application
    Create {
        /// Application name
        name: String,
    },

    /// Show detailed information about an application
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

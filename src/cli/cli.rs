//! Top-level CLI struct and the Commands enum.

use clap::{Parser, Subcommand};

use super::cmds::{AppsCmd, ConfigCmd, HookCmd, PluginCmd, StatsCmd};
use super::container::ContainerSubCmd;

/// riku — the smallest PaaS you've ever seen (Rust edition)
#[derive(Parser, Debug)]
#[command(
    name = "riku",
    version,
    about = "riku — the smallest PaaS you've ever seen (Rust edition)",
    long_about = "riku is a single-binary micro-PaaS that provides Heroku-like git push deployments.\nManage apps, config, processes, and plugins — all from one tool."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List deployed apps
    #[command(
        after_help = "Examples:\n  riku apps\n  riku apps create myapp\n  riku apps info myapp\n  riku apps destroy myapp"
    )]
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
    #[command(
        subcommand,
        after_help = "Examples:\n  riku config show myapp\n  riku config set myapp KEY=val\n  riku config get myapp KEY\n  riku config unset myapp KEY\n  riku config live myapp"
    )]
    Config(ConfigCmd),

    /// Container export and remote deployment commands
    Container {
        #[command(subcommand)]
        cmd: ContainerSubCmd,
    },

    /// Force redeploy of an app
    #[command(
        after_help = "Examples:\n  riku deploy myapp\n  riku deploy myapp --from ./local-path"
    )]
    Deploy {
        /// App name
        app: String,

        /// Deploy from local path instead of git repo
        #[arg(long)]
        from: Option<String>,
    },

    /// Remove an app (preserves data dir)
    #[command(after_help = "Examples:\n  riku destroy myapp")]
    Destroy {
        /// App name
        app: String,
    },

    /// Tail app logs
    #[command(
        after_help = "Examples:\n  riku logs myapp\n  riku logs myapp web\n  riku logs myapp worker\n  riku logs myapp --deploy\n  riku logs myapp --deploy --follow"
    )]
    Logs {
        /// App name
        app: String,
        /// Process filter (default: all)
        #[arg(default_value = "*")]
        process: String,
        /// Show the deploy log instead of runtime process logs
        #[arg(long, short = 'd')]
        deploy: bool,
        /// Follow (tail -f) the deploy log live; only valid with --deploy
        #[arg(long, short = 'f', requires = "deploy")]
        follow: bool,
    },

    /// Manage app processes
    #[command(
        after_help = "Examples:\n  riku ps\n  riku ps myapp\n  riku ps myapp --scale web=2 worker=1\n  riku ps myapp --verbose"
    )]
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
    #[command(
        subcommand,
        after_help = "Examples:\n  riku stats all\n  riku stats app myapp"
    )]
    Stats(StatsCmd),

    /// Run a command in the app context
    #[command(
        trailing_var_arg = true,
        after_help = "Examples:\n  riku run myapp python manage.py shell\n  riku run myapp bash"
    )]
    Run {
        /// App name
        app: String,
        /// Command and arguments to run
        #[arg(required = true)]
        cmd: Vec<String>,
    },

    /// Restart an app (hot reload for zero downtime)
    #[command(after_help = "Examples:\n  riku restart myapp\n  riku restart myapp --hot")]
    Restart {
        /// App name
        app: String,
        /// Use hot reload (zero downtime)
        #[arg(long, short)]
        hot: bool,
    },

    /// Stop an app
    #[command(after_help = "Examples:\n  riku stop myapp")]
    Stop {
        /// App name
        app: String,
    },

    /// Initialize Riku server (directories + systemd + SSH)
    #[command(after_help = "Examples:\n  riku init\n  riku init --no-systemd")]
    Init {
        /// Skip systemd service setup
        #[arg(long)]
        no_systemd: bool,
    },

    /// Self-update the riku binary
    #[command(after_help = "Examples:\n  riku update")]
    Update,

    /// Download and install bundled runtime plugins to ~/.riku/plugins/
    #[command(
        name = "install-plugins",
        after_help = "Examples:\n  riku install-plugins\n  riku install-plugins --plugins node,python"
    )]
    InstallPlugins {
        /// Comma-separated list of plugin names to install (default: all)
        #[arg(long, value_delimiter = ',')]
        plugins: Vec<String>,
    },

    /// Start the process supervisor daemon
    #[command(after_help = "Examples:\n  riku supervisor")]
    Supervisor,

    /// Manage client-side plugins (local scripts that extend riku CLI)
    #[command(
        subcommand,
        about = "Manage client-side plugins (local scripts that extend riku CLI)",
        after_help = "Examples:\n  riku plugin list\n  riku plugin exists riku-deploy"
    )]
    Plugin(PluginCmd),

    /// Manage server-side lifecycle hook plugins
    #[command(
        subcommand,
        about = "Manage server-side lifecycle hook plugins",
        after_help = "Examples:\n  riku hook list\n  riku hook check riku-pre-deploy"
    )]
    Hook(HookCmd),

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

    /// Namespace-isolation shim (internal — exec'd by the supervisor, never
    /// run directly). Reads `RIKU_NS_ROOT` / `RIKU_NS_CMD` from its
    /// environment, sets up the worker's mount/network/PID namespaces, and
    /// execs the real worker command inside them.
    ///
    /// This exists as a separate subcommand (rather than namespace setup
    /// happening inside `pre_exec` before `execve`) so that the fork this
    /// requires for PID-namespace isolation happens *after* the supervisor's
    /// `Command::spawn()` has already returned: `pre_exec` runs before the
    /// CLOEXEC self-pipe `Command` uses to detect successful `execve`
    /// closes, so forking again in there and never exec'ing in the original
    /// process — which the old implementation did, to become a
    /// signal-forwarding shim — left `Command::spawn()` blocked on that pipe
    /// for the worker's entire lifetime, freezing the whole supervisor.
    #[command(hide = true, name = "__ns-shim")]
    NsShim,
}

//! What `rk` runs on the laptop itself, and the errors for what it will not.
//!
//! Nothing here touches a server: `server`, `plugin` and `quickstart` come
//! from riku-remote, and every other command is either forwarded by the
//! caller or refused here.

use anyhow::{anyhow, Error, Result};
use riku_args::cli::Commands;
use riku_args::placement::{client_rule, ClientRule, Elsewhere};
use riku_remote::plugin_cmd::run_plugin;
use riku_remote::quickstart::cmd_quickstart;
use riku_remote::remote::route::is_plugin_authoring;
use riku_remote::remote::server_cmd;
use riku_remote::remote::store;
use riku_remote::remote::target::registered_names;

const NO_LOCAL_MODE: &str = "rk has no local mode; run the command with riku";
const NO_SERVER: &str =
    "no server selected; add one with rk server add <name> <user@host>, or pass -s <name>";

fn not_part_of_rk(command: &str, elsewhere: Elsewhere) -> Error {
    let runner = match elsewhere {
        Elsewhere::OnServer => "riku on the server",
        Elsewhere::FullBinary => "the full riku binary",
    };
    anyhow!("'{command}' is not part of rk; run it with {runner}")
}

/// The error for `--local`, which `rk` has no meaning for.
pub(crate) fn no_local_mode() -> Error {
    anyhow!(NO_LOCAL_MODE)
}

/// Refuses `container` and the server only commands. `dashboard` passes: its
/// remote form runs here.
pub(crate) fn check_supported(command: &str) -> Result<()> {
    match client_rule(command) {
        ClientRule::Refused(elsewhere) => Err(not_part_of_rk(command, elsewhere)),
        ClientRule::RunsHere | ClientRule::Forwards => Ok(()),
    }
}

/// The no server error, naming the registered servers.
fn no_server_selected() -> Error {
    let path = match store::servers_path() {
        Ok(path) => path,
        Err(e) => return anyhow!("{NO_SERVER} ({e})"),
    };
    match store::load(&path) {
        Ok(list) => anyhow!("{NO_SERVER} ({})", registered_names(&list)),
        Err(e) => anyhow!("{NO_SERVER} ({e})"),
    }
}

/// Handles a command that resolved to no server. `name` and `sub` are the
/// clap names of the command and its subcommand. `server`, `plugin` and
/// `quickstart` run here, plugin authoring is refused, and anything else is
/// the no server error.
pub(crate) fn run_without_server(command: Commands, name: &str, sub: Option<&str>) -> Result<()> {
    match command {
        Commands::Server(cmd) => server_cmd::run(cmd),
        Commands::Plugin(cmd) => run_plugin(cmd),
        Commands::Quickstart {
            name: app,
            runtime,
            remote,
        } => cmd_quickstart(&app, &runtime, remote.as_deref()),
        _ if is_plugin_authoring(name, sub) => {
            let refused = match sub {
                Some(sub) => format!("{name} {sub}"),
                None => name.to_string(),
            };
            Err(not_part_of_rk(&refused, Elsewhere::FullBinary))
        }
        _ => Err(no_server_selected()),
    }
}

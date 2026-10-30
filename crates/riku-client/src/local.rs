//! What `rk` runs on the laptop itself, and the errors for what it will not.
//!
//! Nothing here touches a server: `server`, `plugin` and `quickstart` come
//! from riku-remote, and every other command is either forwarded by the
//! caller or refused here.

use anyhow::{anyhow, Error, Result};
use riku_args::cli::Commands;
use riku_args::placement::{placement_of, Placement};
use riku_remote::plugin_cmd::run_plugin;
use riku_remote::quickstart::cmd_quickstart;
use riku_remote::remote::route::is_plugin_authoring;
use riku_remote::remote::server_cmd;
use riku_remote::remote::store;
use riku_remote::remote::target::registered_names;

const NO_LOCAL_MODE: &str = "rk has no local mode; run the command with riku";
const NO_SERVER: &str =
    "no server selected; add one with rk server add <name> <user@host>, or pass -s <name>";
const CONTAINER: &str = "container";
const DASHBOARD: &str = "dashboard";

/// Where a command that `rk` refuses has to be run instead.
pub enum Elsewhere {
    OnServer,
    FullBinary,
}

/// The error for a command that is not part of `rk`.
pub fn not_part_of_rk(command: &str, elsewhere: Elsewhere) -> Error {
    let runner = match elsewhere {
        Elsewhere::OnServer => "riku on the server",
        Elsewhere::FullBinary => "the full riku binary",
    };
    anyhow!("'{command}' is not part of rk; run it with {runner}")
}

/// The error for `--local`, which `rk` has no meaning for.
pub fn no_local_mode() -> Error {
    anyhow!(NO_LOCAL_MODE)
}

/// Refuses `container` and the server only commands. `dashboard` passes: its
/// remote form runs here.
pub fn check_supported(command: &str) -> Result<()> {
    if command == CONTAINER {
        return Err(not_part_of_rk(command, Elsewhere::FullBinary));
    }
    if placement_of(command) == Some(Placement::Server) && command != DASHBOARD {
        return Err(not_part_of_rk(command, Elsewhere::OnServer));
    }
    Ok(())
}

/// The error for a command that needs a server when none was selected,
/// naming the registered ones.
pub fn no_server_selected() -> Error {
    let path = match store::servers_path() {
        Ok(path) => path,
        Err(e) => return anyhow!("{NO_SERVER} ({e})"),
    };
    match store::load(&path) {
        Ok(list) => anyhow!("{NO_SERVER} ({})", registered_names(&list)),
        Err(e) => anyhow!("{NO_SERVER} ({e})"),
    }
}

/// Handles a command that resolved to no server: local commands run, plugin
/// authoring is refused, anything else is the no server error.
pub fn run_without_server(command: Commands, name: &str, sub: Option<&str>) -> Result<()> {
    if placement_of(name) == Some(Placement::Local) {
        return run(command);
    }
    if is_plugin_authoring(name, sub) {
        let refused = match sub {
            Some(sub) => format!("{name} {sub}"),
            None => name.to_string(),
        };
        return Err(not_part_of_rk(&refused, Elsewhere::FullBinary));
    }
    Err(no_server_selected())
}

fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Server(cmd) => server_cmd::run(cmd),
        Commands::Plugin(cmd) => run_plugin(cmd),
        Commands::Quickstart {
            name,
            runtime,
            remote,
        } => cmd_quickstart(&name, &runtime, remote.as_deref()),
        _ => Err(anyhow!("this command needs a server")),
    }
}

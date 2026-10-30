//! Fills an omitted app name from the app hint.
//!
//! The hint is the app of the repo's matching git remote: on the client it
//! comes from target resolution, on the server from `--app-hint`. Only the
//! commands whose app positional is optional take it. `destroy` is not one of
//! them, since it does not prompt and must always name its app.

use crate::cli::Commands;
use crate::cmds::{AddonCmd, ConfigCmd, StatsCmd};

/// The optional app field of commands that take the hint. `ps` is left out:
/// its omitted app means every app.
fn app_slot(command: &mut Commands) -> Option<&mut Option<String>> {
    match command {
        Commands::Deploy { app, .. }
        | Commands::Logs { app, .. }
        | Commands::Restart { app, .. }
        | Commands::Stop { app }
        | Commands::Rollback { app, .. }
        | Commands::Backup { app, .. }
        | Commands::Config(ConfigCmd::Show { app } | ConfigCmd::Live { app })
        | Commands::Stats(StatsCmd::App { app })
        | Commands::Addon(AddonCmd::Bind { app, .. } | AddonCmd::Unbind { app, .. }) => Some(app),
        _ => None,
    }
}

/// Sets the command's omitted app to `hint`. Returns whether it did, which is
/// also whether the hint has to travel with a forwarded command.
pub fn apply_app_hint(command: &mut Commands, hint: Option<&str>) -> bool {
    match (app_slot(command), hint) {
        (Some(slot), Some(hint)) if slot.is_none() => {
            *slot = Some(hint.to_string());
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "app_hint_tests.rs"]
mod tests;

//! Command line definitions for riku, with no server side dependencies.
//!
//! `cli` and `cmds` hold the clap types, `placement` says where each command
//! runs, `help` renders the grouped help, and `app_hint` fills in an omitted
//! app name, and `parse` is the parse and help rerender both binaries use.
//! `address` holds the validated server name and ssh address.
//! Keeping these here lets a laptop only client parse commands without
//! linking the server crates.

pub mod address;
pub mod app_hint;
#[allow(clippy::module_inception)]
pub mod cli;
pub mod cmds;
pub mod container_args;
pub mod help;
pub mod parse;
pub mod placement;

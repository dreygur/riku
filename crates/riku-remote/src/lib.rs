//! Laptop side behaviour shared by the riku and rk binaries.
//!
//! Holds the named server list and command forwarding, the dashboard client,
//! client plugins, quickstart scaffolding and plugin routing. It depends on no
//! server crate, so a client binary built on it stays small.

pub(crate) use riku_args::{cmds, help, placement};
pub(crate) use riku_util as util;

pub mod client_plugins;
pub mod plugin_cmd;
pub mod quickstart;
pub mod remote;
pub mod routing;

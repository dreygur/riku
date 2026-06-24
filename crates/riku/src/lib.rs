// Library crate facade — exposes internal modules for integration tests.
//
// The binary entry point (`main.rs`) remains the sole `fn main`.
// All modules listed here mirror the `mod` declarations in `main.rs`.

pub use riku_cli as cli;
pub use riku_config as config;
pub use riku_dashboard as dashboard;
pub use riku_deploy as deploy;
pub use riku_error as error;
pub use riku_nginx as nginx;
pub use riku_plugins as plugins;
pub use riku_supervisor as supervisor;
pub use riku_util as util;

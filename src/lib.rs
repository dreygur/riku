// Library crate facade — exposes internal modules for integration tests.
//
// The binary entry point (`main.rs`) remains the sole `fn main`.
// All modules listed here mirror the `mod` declarations in `main.rs`.

pub mod cli;
pub mod config;
pub mod deploy;
pub mod nginx;
pub mod plugins;
pub mod supervisor;
pub mod util;

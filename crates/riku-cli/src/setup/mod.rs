//! Setup commands: server initialization, binary installation, systemd, and SSH key management.

pub mod binary;
pub mod git_hook;
mod guidance;
pub mod init;
pub mod ssh;
pub mod system_service;
pub mod systemd;
pub mod user_service;

pub use init::cmd_init;

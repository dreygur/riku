//! Git integration: bare repo setup, post-receive hook, and push handling.

pub mod hook;
mod hook_repo;
mod push_status;
pub mod receive_pack;
pub mod repo;

pub use hook::cmd_git_hook;
pub use receive_pack::{cmd_git_receive_pack, cmd_git_upload_pack};

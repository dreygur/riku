//! Git integration: bare repo setup, post-receive hook, and push handling.

pub mod hook;
pub mod receive_pack;
pub mod repo;

pub use hook::cmd_git_hook;
pub use receive_pack::{cmd_git_receive_pack, cmd_git_upload_pack};
pub use repo::{ensure_repo_symlink, extract_bare_repo_to_app, setup_post_receive_hook};

//! Process status and scaling commands (ps).
//!
//! Split into focused sub-modules:
//! - `ps_all`   — show all processes across all apps
//! - `ps_show`  — show processes for a single app
//! - `ps_scale` — scale workers for an app

pub(super) mod ps_all;
pub(super) mod ps_scale;
pub(super) mod ps_show;

pub use ps_all::cmd_ps_all;
pub use ps_scale::cmd_ps_scale;
pub use ps_show::cmd_ps_show;

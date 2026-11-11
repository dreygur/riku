//! AI Agent Interface
//!
//! Provides SSH-based access for AI agents (Claude, Cursor, Copilot, etc.)
//! to perform deployment and management tasks with proper authentication,
//! authorization, and audit logging.

pub mod audit;
pub mod auth;
pub mod commands;
pub mod confirm;
pub mod execute;
pub mod help;
mod quiet_stdout;
pub mod schema;
mod secret_keys;
pub mod state;
pub mod types;

pub use execute::cmd_agent_execute;
pub use help::cmd_agent_help;
pub use schema::{cmd_agent_intro, cmd_agent_schema};

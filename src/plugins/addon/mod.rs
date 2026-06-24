//! Addon seam — managed resources (databases, caches, queues) as plugins.
//!
//! See `PLUGIN_PROTOCOL.md` §6.1. Layering:
//! - [`state`] — repository: the on-disk instance registry.
//! - [`dispatch`] — runs one addon verb as a child process.
//! - [`service`] — business logic tying discovery, dispatch, state, and app
//!   env injection together.

mod dispatch;
mod service;
mod state;

pub use service::AddonService;
pub use state::{InstanceRecord, InstanceStore};

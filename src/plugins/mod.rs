//! Plugin system for Riku.
//!
//! ## Architecture
//!
//! The plugin system has two layers:
//!
//! **1. Discovery & execution primitives** (`discovery` module)
//! Low-level functions for finding and running individual plugins by name.
//!
//! **2. Lifecycle hook manager** (`manager` module)
//! High-level `PluginManager` that fires standard hooks at defined points in
//! the deploy pipeline. See [`hooks::PluginHook`] for available hooks.
//!
//! ## Quick start — writing a plugin
//!
//! ```sh
//! # ~/.riku/plugins/riku-post-deploy
//! #!/bin/bash
//! set -e
//! echo "[$RIKU_APP] Deployed at $(date)"
//! # Run database migrations
//! cd "$RIKU_APP_PATH" && ./manage.py migrate --run-syncdb
//! ```
//!
//! ```sh
//! chmod +x ~/.riku/plugins/riku-post-deploy
//! ```
//!
//! ## Available hooks
//!
//! | Hook name      | Plugin file         | Fires when                         |
//! |----------------|---------------------|------------------------------------|
//! | `pre-deploy`   | `riku-pre-deploy`   | After env load, before build       |
//! | `pre-build`    | `riku-pre-build`    | Before the runtime build step      |
//! | `post-build`   | `riku-post-build`   | After build, before workers start  |
//! | `post-deploy`  | `riku-post-deploy`  | After workers are started          |

/// The plugin protocol version this kernel implements. Set in every plugin
/// invocation's environment as `RIKU_PLUGIN_API` and stamped into every event
/// envelope. See `PLUGIN_PROTOCOL.md` §2.
pub const RIKU_PLUGIN_API: u32 = 1;

#[allow(unused_imports)]
pub mod discovery;
pub mod events;
pub(crate) mod executor;
pub mod hooks;
pub mod manager;
pub mod manifest;
pub mod runtime;

// Re-export the public API (used by CLI plugin commands and external code)
#[allow(unused_imports)]
pub use discovery::{list_plugins, plugin_exists};
pub use events::{EventBus, EventEnvelope, EventName};
pub use hooks::{HookContext, PluginHook};
pub use manager::PluginManager;
pub use manifest::PluginManifest;

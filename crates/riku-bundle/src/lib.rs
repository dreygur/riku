//! What a riku plugin bundle is and how it is signed.
//!
//! A bundle is a directory with a `riku-plugin.toml` manifest and the entry
//! executable it names. [`manifest`] parses and validates the manifest;
//! [`signing`] holds the author's Ed25519 keypair and signature check. The
//! crate does no I/O beyond reading a manifest, and knows nothing about
//! servers, so both `riku` and the laptop only `rk` can use it.
//!
//! Security model: a manifest is attacker influenced data and is validated
//! before anything uses it. A signature proves a key holder signed the entry
//! bytes. It says nothing about who that holder is, and nothing about files
//! in the bundle other than the entry. Trust decisions live with the caller.

pub mod manifest;
pub mod signing;

pub use manifest::PluginManifest;

/// The plugin protocol version this kernel implements. Set in every plugin
/// invocation's environment as `RIKU_PLUGIN_API` and stamped into every event
/// envelope. See `PLUGIN_PROTOCOL.md` §2.
pub const RIKU_PLUGIN_API: u32 = 1;

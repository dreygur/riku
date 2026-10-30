//! The `[router]` table of a router plugin's manifest.

use serde::Deserialize;

/// What a router plugin declares about itself (`PLUGIN_PROTOCOL.md` section 6.2).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RouterSection {
    /// The router protocol the plugin speaks. Riku only drives plugins whose
    /// value equals [`crate::ROUTER_PROTOCOL`].
    pub protocol: u32,
    /// True when `reload` swaps configuration without dropping connections.
    /// Rolling deploys drain old copies only behind a graceful router.
    #[serde(default)]
    pub graceful_reload: bool,
}

#[cfg(test)]
#[path = "router_section_tests.rs"]
mod tests;

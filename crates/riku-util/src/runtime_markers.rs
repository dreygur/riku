//! The runtime plugins riku ships, and the repo files that tell which one an
//! app needs.
//!
//! Lives in riku-util because both the server (riku-deploy, riku-cli) and the
//! laptop client use it, and the client may not link the server crates.

/// Every runtime plugin riku can install by name: the five shell plugins
/// built into riku, then the java and clojure binaries it downloads.
pub const BUNDLED_RUNTIMES: &[&str] = &[
    "go",
    "node",
    "python",
    "ruby",
    "rust-lang",
    "java",
    "clojure",
];

/// Whether `name` is one of [`BUNDLED_RUNTIMES`].
pub fn is_bundled_runtime(name: &str) -> bool {
    BUNDLED_RUNTIMES.contains(&name)
}

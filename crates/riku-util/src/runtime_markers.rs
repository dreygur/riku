//! The runtime plugins riku ships, and the repo files that tell which one an
//! app needs.
//!
//! Lives in riku-util because both the server (riku-deploy, riku-cli) and the
//! laptop client use it, and the client may not link the server crates.

use std::path::Path;

/// Repo files that mark each shell runtime, in the order server detection
/// tries the plugins: alphabetical, first match wins.
const MARKERS: &[(&str, &[&str])] = &[
    ("go", &["go.mod"]),
    ("node", &["package.json"]),
    (
        "python",
        &["requirements.txt", "pyproject.toml", "setup.py"],
    ),
    ("ruby", &["Gemfile"]),
    ("rust-lang", &["Cargo.toml"]),
];

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

/// The shell runtime whose marker file `app_path` has, if any. Used to name
/// the plugin to install when no installed plugin accepted the app.
pub fn needed_runtime(app_path: &Path) -> Option<&'static str> {
    MARKERS
        .iter()
        .find(|(_, files)| files.iter().any(|file| app_path.join(file).exists()))
        .map(|(runtime, _)| *runtime)
}

#[cfg(test)]
#[path = "runtime_markers_tests.rs"]
mod tests;

//! Where riku deploy keys live and how riku recognises its own lines.

use std::path::{Path, PathBuf};

/// Marks a forced command written by riku, as opposed to a foreign key.
pub const RIKU_KEY_MARKER: &str = "FINGERPRINT=";

/// Where sshd looks for the authorized keys of the user whose home is `home`.
pub fn authorized_keys_path(home: &Path) -> PathBuf {
    home.join(".ssh").join("authorized_keys")
}

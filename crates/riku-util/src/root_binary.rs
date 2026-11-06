//! The one place riku is installed on a server.

/// Where root init installs riku, root owned and closed to group and other
/// writers. Every unit, git hook and forced command riku writes names this
/// path, never one under the deploy user's home. A hook or `authorized_keys`
/// line is still a file in the deploy user's tree, so the deploy user can
/// edit it; what it then runs is run with the deploy user's privilege.
pub const ROOT_BINARY: &str = "/usr/local/bin/riku";

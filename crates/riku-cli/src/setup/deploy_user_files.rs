//! Reads root init makes inside the deploy user's tree, each run as that
//! user so a planted symlink can only lead where the user could already go.

use std::path::Path;

use anyhow::Result;

use super::DeployUser;

impl DeployUser {
    /// Read `path` as the deploy user.
    pub fn read_file(&self, path: &Path) -> Result<String> {
        self.run_as("cat", &[&path.to_string_lossy()])
    }
}

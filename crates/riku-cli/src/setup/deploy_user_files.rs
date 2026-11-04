//! Changes root init makes inside the deploy user's tree, each run as that
//! user so a planted symlink can only lead where the user could already go.

use std::path::Path;

use anyhow::Result;

use super::DeployUser;

impl DeployUser {
    /// Create `dir` and its parents as the deploy user.
    pub fn create_dir(&self, dir: &Path) -> Result<()> {
        self.run_as("mkdir", &["-p", &dir.to_string_lossy()])
            .map(drop)
    }

    /// Change `path`'s mode as the deploy user (`mode` in chmod syntax).
    pub fn chmod(&self, mode: &str, path: &Path) -> Result<()> {
        self.run_as("chmod", &[mode, &path.to_string_lossy()])
            .map(drop)
    }

    /// Read `path` as the deploy user.
    pub fn read_file(&self, path: &Path) -> Result<String> {
        self.run_as("cat", &[&path.to_string_lossy()])
    }
}

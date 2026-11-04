//! The programs riku runs to reload the host router.

use std::path::PathBuf;

use crate::program::find_program;

/// Programs a router reload runs, looked up on `PATH` in production. Tests
/// replace them with scripts in their own directory, so no test has to
/// change `PATH`.
#[derive(Debug, Clone)]
pub struct ReloadPrograms {
    pub systemctl: PathBuf,
    pub caddy: PathBuf,
    pub nginx: PathBuf,
}

impl Default for ReloadPrograms {
    fn default() -> Self {
        ReloadPrograms {
            systemctl: find_program("systemctl"),
            caddy: find_program("caddy"),
            nginx: find_program("nginx"),
        }
    }
}

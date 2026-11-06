//! Doctor checks for the root owned riku binary. Read-only.

use std::path::Path;

use crate::setup::binary::check_installed;
use crate::util::write_dir::ROOT_UID;
use crate::util::ROOT_BINARY;

use super::Check;

fn installed(path: &Path) -> Check {
    match check_installed(path, ROOT_UID) {
        Ok(()) => Check::ok("installed binary", path.display().to_string()),
        Err(e) => Check::fail("installed binary", e.to_string()),
    }
}

/// Whether a shell finds riku and whether [`ROOT_BINARY`] is safely
/// installed.
///
/// Hooks, units and forced commands run the binary by its full path, so a
/// PATH without riku is fine. A different riku first on PATH is a failure:
/// it is what `riku doctor --fix` would run.
pub fn binary() -> Vec<Check> {
    let on_path = match which::which("riku") {
        Ok(p) if p == Path::new(ROOT_BINARY) => Check::ok("riku on PATH", p.display().to_string()),
        Ok(p) => Check::fail(
            "riku on PATH",
            format!(
                "{} shadows {ROOT_BINARY}. Remove it or run sudo riku init",
                p.display()
            ),
        ),
        Err(_) => Check::ok(
            "riku on PATH",
            "not on this PATH; hooks and services use the full path",
        ),
    };
    vec![on_path, installed(Path::new(ROOT_BINARY))]
}

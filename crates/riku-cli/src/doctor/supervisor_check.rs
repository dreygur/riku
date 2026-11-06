//! `riku doctor` check that the running supervisor was started from the
//! installed binary and that binary has not been replaced since.
//!
//! Read-only: it reads the supervisor's pid file and the `exe` link under
//! the process directory. Both roots are parameters so tests use temp dirs.

use std::fs;
use std::path::Path;

use crate::util::ROOT_BINARY;

use super::Check;

const NAME: &str = "supervisor binary";
const FIX: &str = "sudo systemctl restart riku";
const DELETED_SUFFIX: &str = " (deleted)";

/// Compare `<proc_root>/<pid>/exe` of the pid in `pid_file` with
/// [`ROOT_BINARY`]. No pid file means no supervisor is running, which the
/// service check reports; this one stays quiet then.
pub fn supervisor_binary(pid_file: &Path, proc_root: &Path) -> Vec<Check> {
    let text = match fs::read_to_string(pid_file) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };
    let pid: u32 = match text.trim().parse() {
        Ok(pid) => pid,
        Err(_) => {
            return vec![Check::warn(
                NAME,
                format!("{} does not hold a pid", pid_file.display()),
            )]
        }
    };
    let exe = match fs::read_link(proc_root.join(pid.to_string()).join("exe")) {
        Ok(exe) => exe.to_string_lossy().into_owned(),
        Err(e) => {
            return vec![Check::warn(
                NAME,
                format!("cannot read the binary of supervisor pid {pid}: {e}"),
            )]
        }
    };
    if exe == ROOT_BINARY {
        return vec![Check::ok(NAME, format!("pid {pid} runs {ROOT_BINARY}"))];
    }
    let reason = match exe.strip_suffix(DELETED_SUFFIX) {
        Some(_) => format!("pid {pid} runs {exe}, a binary that was replaced"),
        None => format!("pid {pid} runs {exe}, not {ROOT_BINARY}"),
    };
    vec![Check::fail(NAME, format!("{reason}. Run: {FIX}"))]
}

#[cfg(test)]
#[path = "supervisor_check_tests.rs"]
mod tests;

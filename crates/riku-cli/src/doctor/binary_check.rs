//! Doctor checks for the root owned riku binary and for anything that
//! still starts riku from another path. Read-only.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::config::RikuPaths;
use crate::setup::system_service::SYSTEMD_DIR;
use crate::util::authorized_keys::authorized_keys_path;
use crate::util::ROOT_BINARY;

use super::Check;

const ROOT_UID: u32 = 0;

/// Lines that start riku: a unit's `ExecStart=`, a deploy key's
/// `command="`, and a hook's `BINARY=`.
const LAUNCH_MARKERS: [&str; 3] = ["ExecStart=", "command=\"", "BINARY="];
const EXEC_BITS: u32 = 0o111;
/// Init rewrites none of the files that can name a stale path, so the
/// remedy is to change them.
const STALE_REMEDY: &str = "no migration: reinstall and redeploy, or edit the named files";

/// A file root or the deploy user installs and whose launch line must name
/// [`ROOT_BINARY`].
fn launch_files(paths: &RikuPaths) -> Vec<PathBuf> {
    let mut files = vec![
        Path::new(SYSTEMD_DIR).join("riku.service"),
        paths.riku_root.join("hooks/post-receive"),
    ];
    if let Some(home) = paths.riku_root.parent() {
        files.push(authorized_keys_path(home));
        files.push(home.join(".config/systemd/user/riku.service"));
    }
    if let Ok(repos) = fs::read_dir(&paths.git_root) {
        files.extend(
            repos
                .flatten()
                .map(|repo| repo.path().join("hooks/post-receive")),
        );
    }
    files
}

/// Words on a launch line that name a riku binary other than the root one.
fn stray_binaries(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| LAUNCH_MARKERS.iter().any(|marker| line.contains(marker)))
        .flat_map(|line| line.split(|c: char| c.is_whitespace() || c == '"' || c == '='))
        .filter(|word| word.ends_with("/riku") && *word != ROOT_BINARY)
        .map(str::to_string)
        .collect()
}

fn installed(path: &Path, owner: u32) -> Check {
    let meta = match path.symlink_metadata() {
        Ok(meta) => meta,
        Err(_) => {
            return Check::fail(
                "installed binary",
                format!("{} not found, run: sudo riku init", path.display()),
            )
        }
    };
    if meta.uid() != owner {
        return Check::fail(
            "installed binary",
            format!(
                "{} is not root owned: the deploy user could replace it",
                path.display()
            ),
        );
    }
    if meta.mode() & 0o022 != 0 {
        return Check::fail(
            "installed binary",
            format!(
                "{} is group or world writable (mode {:o})",
                path.display(),
                meta.mode() & 0o777
            ),
        );
    }
    if meta.mode() & EXEC_BITS == 0 {
        return Check::fail(
            "installed binary",
            format!("{} is not executable", path.display()),
        );
    }
    Check::ok("installed binary", path.display().to_string())
}

fn strays(files: &[PathBuf]) -> Check {
    let found: Vec<String> = files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| {
            stray_binaries(&text)
                .into_iter()
                .map(move |bin| format!("{} runs {bin}", file.display()))
        })
        .collect();
    if found.is_empty() {
        return Check::ok("launch paths", format!("all name {ROOT_BINARY}"));
    }
    Check::fail(
        "launch paths",
        format!("{}; {STALE_REMEDY}", found.join(", ")),
    )
}

/// Whether a shell finds riku, whether [`ROOT_BINARY`] is safely installed,
/// and whether any unit, hook or deploy key still names another path.
///
/// PATH is informational only: hooks, units and forced commands run the
/// binary by its full path.
pub fn binary(paths: &RikuPaths) -> Vec<Check> {
    let on_path = match which::which("riku") {
        Ok(p) => Check::ok("riku on PATH", p.display().to_string()),
        Err(_) => Check::ok(
            "riku on PATH",
            "not on this PATH; hooks and services use the full path",
        ),
    };
    vec![
        on_path,
        installed(Path::new(ROOT_BINARY), ROOT_UID),
        strays(&launch_files(paths)),
    ]
}

#[cfg(test)]
#[path = "binary_check_tests.rs"]
mod tests;

//! Refusing spec values that the engine would read as something other than data.
//!
//! Security model: the spec comes from a compose file. Most values travel as
//! the value of a flag (`--user x`) and cannot become a flag themselves. The
//! exceptions are the image, which is positional, and the colon separated
//! `-v` and `--label` forms, where an extra `:` or `=` changes what is mounted
//! or labelled, and `--mount`, whose value the engine parses as CSV and cuts
//! at the first newline. Those are checked here before any argument is built.

use std::path::Path;

use anyhow::{anyhow, Result};

use super::run_spec::RunSpec;
use crate::types::{HealthTest, Mount, MountSource};

/// An image reference or build context that is not empty, does not start with
/// `-` and holds no whitespace or control characters.
pub(crate) fn check_positional(what: &str, value: &str) -> Result<()> {
    let bad = value.is_empty()
        || value.starts_with('-')
        || value.chars().any(|c| c.is_whitespace() || c.is_control());
    if bad {
        return Err(anyhow!(
            "{what} {value:?} is not allowed: it must be non-empty, not start with '-', and hold no whitespace"
        ));
    }
    Ok(())
}

fn check_absolute(what: &str, path: &Path) -> Result<()> {
    let text = path.to_string_lossy();
    let bad_char = |c: char| c == ':' || c.is_whitespace() || c.is_control();
    if !path.is_absolute() || text.chars().any(bad_char) {
        return Err(anyhow!(
            "{what} {text:?} must be an absolute path without ':', whitespace or control characters"
        ));
    }
    Ok(())
}

fn check_volume_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    if !first_ok || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) {
        return Err(anyhow!(
            "volume name {name:?} is not allowed: use letters, digits, '_', '.' or '-', starting with a letter or digit"
        ));
    }
    Ok(())
}

fn check_mount(mount: &Mount) -> Result<()> {
    match check_absolute("mount target", &mount.target) {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    if mount.target.to_string_lossy().contains(',') {
        return Err(anyhow!(
            "mount target {:?} must not hold ',', which would add mount options",
            mount.target
        ));
    }
    match &mount.source {
        MountSource::Volume(name) => check_volume_name(name),
        MountSource::Bind(path) => check_absolute("bind mount source", path),
        MountSource::Anonymous => Ok(()),
    }
}

/// Check every value of `spec` that is not safe as a plain flag value.
pub(crate) fn check_spec(spec: &RunSpec) -> Result<()> {
    match check_positional("image", &spec.image) {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    if let Some(key) = spec.labels.keys().find(|k| k.is_empty() || k.contains('=')) {
        return Err(anyhow!("label key {key:?} is empty or holds '='"));
    }
    match spec.mounts.iter().try_for_each(check_mount) {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    if let Some(HealthTest::Exec(argv)) = spec.healthcheck.as_ref().map(|h| &h.test) {
        match exec_command_line(argv) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Join an exec form health check into the shell line the engine CLI takes.
///
/// Refuses an empty command, which would silently keep the image's own check,
/// and an argument the quoting cannot represent (a NUL character).
pub(crate) fn exec_command_line(argv: &[String]) -> Result<String> {
    if argv.is_empty() {
        return Err(anyhow!("health check command is empty"));
    }
    match shlex::try_join(argv.iter().map(String::as_str)) {
        Ok(line) => Ok(line),
        Err(e) => Err(anyhow!("health check command cannot be quoted: {e}")),
    }
}

#[cfg(test)]
#[path = "spec_check_tests.rs"]
mod tests;

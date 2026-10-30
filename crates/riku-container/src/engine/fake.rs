//! Fake engine executables for tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Write an executable `/bin/sh` script named `name` into `dir` and return its path.
pub fn fake_engine(dir: &Path, name: &str, script: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

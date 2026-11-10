//! `riku deploy-key add '<key>'`: authorize one deploy key on this server.
//!
//! `rk server add --admin` runs this over the admin login with sudo, so a
//! developer gets push access without copying files by hand.
//!
//! Security: only root may run it, since it grants push access. It takes
//! exactly one key line and keeps only the key type and blob, dropping any
//! `authorized_keys` options and the comment. Root never writes the deploy
//! user's `authorized_keys` itself: it reruns the root owned riku as the
//! deploy user (see [`DeployUser::run_relayed`]), which adds the key forced
//! through riku like `riku init --ssh-key` does.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::util::public_key::{parse_public_keys, PublicKey};
use crate::util::ROOT_BINARY;

use super::deploy_user::{running_as_root, DeployUser};

const NOT_ROOT: &str = "adding a deploy key needs root: sudo riku deploy-key add '<key>'";

/// The one key in `key_line`, as type and blob.
#[allow(clippy::question_mark)]
fn single_key(key_line: &str) -> Result<PublicKey> {
    let mut keys = match parse_public_keys(key_line) {
        Ok(keys) => keys,
        Err(e) => return Err(e),
    };
    if keys.len() != 1 {
        return Err(anyhow!(
            "expected exactly one key, got {}; add each key on its own",
            keys.len()
        ));
    }
    Ok(keys.remove(0))
}

/// Authorize `key_line` for the deploy user. `is_root` is the caller's
/// effective uid check, passed in so the refusal is testable.
#[allow(clippy::question_mark)]
pub(crate) fn add_deploy_key(is_root: bool, key_line: &str) -> Result<()> {
    if !is_root {
        return Err(anyhow!(NOT_ROOT));
    }
    let key = match single_key(key_line) {
        Ok(key) => key,
        Err(e) => return Err(e),
    };
    let user = match DeployUser::lookup() {
        Ok(user) => user,
        Err(e) => return Err(e),
    };
    let key = key.to_string();
    let args = [
        "init",
        "--user-steps",
        "--keys-only",
        "--add-key",
        key.as_str(),
    ];
    user.run_relayed(Path::new(ROOT_BINARY), &args)
}

/// `riku deploy-key add '<key>'`.
pub fn cmd_deploy_key_add(key_line: &str) -> Result<()> {
    add_deploy_key(running_as_root(), key_line)
}

#[cfg(test)]
#[path = "deploy_key_cmd_tests.rs"]
mod tests;

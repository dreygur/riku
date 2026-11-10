//! `riku deploy-key add '<key>'`: authorize one deploy key on this server.
//!
//! `rk server add --admin` runs this over the admin login with sudo, so a
//! developer gets push access without copying files by hand.
//!
//! Security: only root may run it, since it grants push access. The target
//! account must have a real `~/.riku` directory. It takes
//! exactly one key line and keeps only the key type and blob, dropping any
//! `authorized_keys` options and the comment. Root never writes the deploy
//! user's `authorized_keys` itself: it reruns the root owned riku as the
//! deploy user (see [`DeployUser::run_relayed`]), which adds the key forced
//! through riku like `riku init --ssh-key` does.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::util::public_key::{parse_public_keys, PublicKey};
use crate::util::ROOT_BINARY;

use super::deploy_user::{deploy_user_name, running_as_root, DeployUser};

const NOT_ROOT: &str = "adding a deploy key needs root: sudo riku deploy-key add '<key>'";
const RIKU_DIR: &str = ".riku";

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

/// Whether `home` holds a real `.riku` directory, not a symlink: the mark
/// of an account `riku init` set up. `symlink_metadata` does not follow
/// links, so the check is safe to make as root.
fn has_riku_tree(home: &Path) -> bool {
    std::fs::symlink_metadata(home.join(RIKU_DIR)).is_ok_and(|meta| meta.is_dir())
}

/// Authorize `key_line` for the deploy account `user_name`. `is_root` is the
/// caller's effective uid check, passed in so the refusal is testable. Only
/// an account with a riku tree is accepted, so the forced riku command never
/// lands on an unrelated account.
#[allow(clippy::question_mark)]
pub(crate) fn add_deploy_key(is_root: bool, user_name: &str, key_line: &str) -> Result<()> {
    if !is_root {
        return Err(anyhow!(NOT_ROOT));
    }
    let key = match single_key(key_line) {
        Ok(key) => key,
        Err(e) => return Err(e),
    };
    let user = match DeployUser::lookup_named(user_name.to_string()) {
        Ok(user) => user,
        Err(e) => return Err(e),
    };
    if !has_riku_tree(&user.home) {
        return Err(anyhow!(
            "{user_name} is not a riku deploy user: {} has no {RIKU_DIR}. Run sudo riku init first",
            user.home.display()
        ));
    }
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

/// `riku deploy-key add [--user NAME] '<key>'`. Without `--user`, the
/// `RIKU_USER` account, else `riku`; sudo drops `RIKU_USER`, so `rk` always
/// passes the user from the server address.
pub fn cmd_deploy_key_add(user: Option<&str>, key_line: &str) -> Result<()> {
    let user = user.map_or_else(deploy_user_name, str::to_string);
    add_deploy_key(running_as_root(), &user, key_line)
}

#[cfg(test)]
#[path = "deploy_key_cmd_tests.rs"]
mod tests;

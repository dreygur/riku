//! Lets nginx's worker processes reach the uWSGI sockets in `~/.riku/nginx`.
//!
//! uWSGI copies create their socket `0660` owned by the deploy user and its
//! primary group (riku passes `UWSGI_CHMOD_SOCKET=660`). Root init adds the
//! nginx worker user to that group and gives the group traverse (not list)
//! access to the home, the riku root and the socket directory. Nothing is
//! made world writable.
//!
//! Security: the worker user name comes from root-owned `nginx.conf` and is
//! checked to be a plain account name before it reaches `usermod`, which
//! runs with a fixed argv and no shell. Changes inside the deploy user's
//! home run as that user through [`DeployUser`].

use std::fs;

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::config::RikuPaths;
use crate::util::echo;

use super::caddy_host::run;
use super::deploy_user::DeployUser;

const NGINX_CONF: &str = "/etc/nginx/nginx.conf";
const DEBIAN_WORKER: &str = "www-data";
const RPM_WORKER: &str = "nginx";

/// The account named by the main-context `user` directive of `conf`.
fn configured_user(conf: &str) -> Option<String> {
    conf.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .find_map(|line| {
            let mut words = line.trim_end_matches(';').split_whitespace();
            match (words.next(), words.next()) {
                (Some("user"), Some(name)) => Some(name.to_string()),
                _ => None,
            }
        })
}

fn is_account_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// The nginx worker user: `nginx.conf`'s `user`, else `www-data` where that
/// account exists (Debian, Ubuntu), else `nginx` (RHEL, Fedora, Arch).
fn worker_user() -> Result<String> {
    let configured = fs::read_to_string(NGINX_CONF)
        .ok()
        .and_then(|conf| configured_user(&conf));
    let name = match configured {
        Some(name) => name,
        None => match User::from_name(DEBIAN_WORKER) {
            Ok(Some(_)) => DEBIAN_WORKER.to_string(),
            _ => RPM_WORKER.to_string(),
        },
    };
    if !is_account_name(&name) {
        return Err(anyhow!("{NGINX_CONF} names an odd worker user {name:?}"));
    }
    Ok(name)
}

/// Put the nginx worker user in the deploy user's group and let that group
/// traverse down to the socket directory. Root only. Not unit tested: it
/// needs root and real accounts, so running init on a host covers it.
#[allow(clippy::question_mark)]
pub(super) fn grant_nginx_sockets(user: &DeployUser, paths: &RikuPaths) -> Result<()> {
    let worker = match worker_user() {
        Ok(worker) => worker,
        Err(e) => return Err(e),
    };
    if let Err(e) = run("usermod", &["-aG", &user.group, &worker]) {
        return Err(e);
    }
    for dir in [&user.home, &paths.riku_root, &paths.nginx_root] {
        if let Err(e) = user.chmod("g+x", dir) {
            return Err(e);
        }
    }
    echo(
        &format!("      ✓ nginx ({worker}) can reach the app sockets"),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "nginx_worker_tests.rs"]
mod tests;

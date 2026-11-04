//! Lets nginx's worker processes reach the uWSGI sockets in
//! `/run/riku-sockets`.
//!
//! uWSGI copies create their socket `0660` in that setgid directory, so each
//! socket belongs to the `riku-sockets` group. Root init adds the nginx
//! worker user to that group and to nothing else: nginx gets no access to
//! the deploy user's home or files.
//!
//! Security: the worker user name comes from root-owned `nginx.conf` and is
//! checked to be a plain account name before it reaches `usermod`, which
//! runs with a fixed argv and no shell. A worker of root, `nobody` or
//! primary gid 0 is refused: root and gid 0 are not a dedicated account,
//! and `nobody` is shared by unrelated daemons, which would all get the
//! sockets along with nginx.

use std::fs;

use anyhow::{anyhow, Result};
use nix::unistd::User;

use crate::config::SOCKET_GROUP;
use crate::util::echo;

use super::socket_group::{add_to_socket_group, is_account_name};

const NGINX_CONF: &str = "/etc/nginx/nginx.conf";
const DEBIAN_WORKER: &str = "www-data";
const RPM_WORKER: &str = "nginx";
const REFUSED_WORKERS: [&str; 2] = ["root", "nobody"];

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

/// Refuses a worker account the socket group must not be given to.
fn check_worker(name: &str, uid: u32, gid: u32) -> Result<()> {
    if !is_account_name(name) {
        return Err(anyhow!("{NGINX_CONF} names an odd worker user {name:?}"));
    }
    if REFUSED_WORKERS.contains(&name) || uid == 0 || gid == 0 {
        return Err(anyhow!(
            "nginx runs its workers as {name} (uid {uid}, gid {gid}); riku will not give \
             that account its app sockets. Set `user www-data;` (or another dedicated \
             account) in {NGINX_CONF} and re-run: sudo riku init"
        ));
    }
    Ok(())
}

/// The nginx worker user: `nginx.conf`'s `user`, else `www-data` where that
/// account exists (Debian, Ubuntu), else `nginx` (RHEL, Fedora, Arch).
#[allow(clippy::question_mark)]
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
    let account = match User::from_name(&name) {
        Ok(Some(account)) => account,
        Ok(None) => return Err(anyhow!("nginx worker user {name} does not exist")),
        Err(e) => return Err(anyhow!("failed to look up nginx worker user {name}: {e}")),
    };
    if let Err(e) = check_worker(&name, account.uid.as_raw(), account.gid.as_raw()) {
        return Err(e);
    }
    Ok(name)
}

/// Put the nginx worker user in the socket group, which root init has
/// already created. Root only.
#[allow(clippy::question_mark)]
pub(super) fn grant_nginx_sockets() -> Result<()> {
    let worker = match worker_user() {
        Ok(worker) => worker,
        Err(e) => return Err(e),
    };
    if let Err(e) = add_to_socket_group(&worker) {
        return Err(e);
    }
    echo(
        &format!("      ✓ nginx ({worker}) can reach the app sockets through {SOCKET_GROUP}"),
        "green",
    );
    Ok(())
}

#[cfg(test)]
#[path = "nginx_worker_tests.rs"]
mod tests;

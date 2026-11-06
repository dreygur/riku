//! `riku router-helper`: the root process that installs router config on
//! behalf of the deploy user.
//!
//! Security model: runs as root and refuses to start as anyone else. It
//! trusts only the deploy user and root, decided in `riku_route::server`
//! from the kernel's peer credentials, and the socket it serves is group
//! restricted to the deploy user's primary group. The deploy user is read
//! from the passwd database (`RIKU_USER`, default `riku`) when the helper
//! starts, not from a request. A request names an app, which is validated as
//! an app name before use; nothing a request carries is a path.
//!
//! `apply` and `remove` install and remove the config of the router the
//! request names, through `riku_route::answer`. Each router runs its route
//! gate again and writes only under its own directory.
//!
//! The helper reads no file the deploy user can write.

use std::path::Path;

use anyhow::{anyhow, Result};
use nix::unistd::Uid;
use riku_route::{answer, bind_socket, restrict_umask, run, CaddyRoots, NginxRoots};

use crate::config::ROUTER_SOCKET;
use crate::setup::deploy_user::DeployUser;

fn not_root(euid: Uid) -> Option<anyhow::Error> {
    if euid.is_root() {
        return None;
    }
    Some(anyhow!(
        "riku router-helper must run as root; it installs router config the deploy user cannot write"
    ))
}

/// Serve the router helper socket until the process is stopped.
pub fn cmd_router_helper() -> Result<()> {
    if let Some(refusal) = not_root(Uid::effective()) {
        return Err(refusal);
    }
    restrict_umask();
    let deploy = match DeployUser::lookup() {
        Ok(deploy) => deploy,
        Err(e) => return Err(anyhow!("router helper needs the deploy user: {e}")),
    };
    let listener = match bind_socket(Path::new(ROUTER_SOCKET), deploy.gid()) {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("router helper socket: {e}")),
    };
    let nginx = NginxRoots::production();
    let caddy = CaddyRoots::production();
    run(&listener, deploy.uid(), &mut |request| {
        answer(&nginx, &caddy, request)
    })
}

#[cfg(test)]
#[path = "router_helper_tests.rs"]
mod tests;

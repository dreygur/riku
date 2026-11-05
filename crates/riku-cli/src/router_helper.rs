//! `riku router-helper`: the root process that will install router config on
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
//! Until the router config is installed through the helper, `apply` and
//! `remove` answer with a render failure and change nothing.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use nix::unistd::Uid;
use riku_container::types::AppName;
use riku_route::{activated_listener, bind_socket, restrict_umask, run, Reply, Request, Stage};

use crate::config::ROUTER_SOCKET;
use crate::setup::deploy_user::DeployUser;

/// How long a socket activated helper waits with no connection before it
/// exits and leaves the socket to start it again.
const IDLE_EXIT: Duration = Duration::from_secs(60);

fn not_root(euid: Uid) -> Option<anyhow::Error> {
    if euid.is_root() {
        return None;
    }
    Some(anyhow!(
        "riku router-helper must run as root; it installs router config the deploy user cannot write"
    ))
}

fn failed(stage: Stage, error: impl ToString) -> Reply {
    Reply::Failed {
        stage,
        error: error.to_string(),
    }
}

fn app_checked(app: &str) -> std::result::Result<AppName, Reply> {
    match AppName::try_from(app) {
        Ok(app) => Ok(app),
        Err(e) => Err(failed(Stage::Schema, e)),
    }
}

fn handle(request: Request) -> Reply {
    let app = match &request {
        Request::Apply { app, .. } | Request::Remove { app } => app_checked(app),
        Request::Status {} => {
            return Reply::Status {
                version: env!("CARGO_PKG_VERSION").to_string(),
            }
        }
    };
    match app {
        Ok(app) => failed(
            Stage::Render,
            format!("the helper does not install router config for '{app}' yet"),
        ),
        Err(reply) => reply,
    }
}

/// Serve the router helper socket until the process is stopped, or until it
/// has been idle for [`IDLE_EXIT`] when systemd started it for a connection.
pub fn cmd_router_helper() -> Result<()> {
    if let Some(refusal) = not_root(Uid::effective()) {
        return Err(refusal);
    }
    restrict_umask();
    let deploy = match DeployUser::lookup() {
        Ok(deploy) => deploy,
        Err(e) => return Err(anyhow!("router helper needs the deploy user: {e}")),
    };
    let activated = match activated_listener() {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("router helper socket activation: {e}")),
    };
    let (listener, idle) = match activated {
        Some(listener) => (listener, Some(IDLE_EXIT)),
        None => match bind_socket(Path::new(ROUTER_SOCKET), deploy.gid()) {
            Ok(listener) => (listener, None),
            Err(e) => return Err(anyhow!("router helper socket: {e}")),
        },
    };
    run(&listener, idle, deploy.uid(), &mut handle)
}

#[cfg(test)]
#[path = "router_helper_tests.rs"]
mod tests;

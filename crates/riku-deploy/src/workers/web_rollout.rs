//! Deploying the web facing kinds of a Procfile (`web` and the wsgi
//! variants): their copies are replaced one at a time through
//! [`ProcessRuntime`], and the router follows each step.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use riku_container::compose::ReplaceOrder;
use riku_util::backend::Backend;

use super::ports::pinned_port;
use super::process_runtime::ProcessRuntime;
use super::web_env::is_wsgi_kind;
use super::WEB_KIND;
use crate::config::RikuPaths;
use crate::rolling::{replace_all, CopyRuntime, RouteSink};

/// The app a deploy is for, shared by every kind it rolls.
pub(super) struct WebDeploy<'a> {
    pub app: &'a str,
    pub app_path: &'a Path,
    pub env: &'a HashMap<String, String>,
    pub paths: &'a RikuPaths,
}

impl<'a> WebDeploy<'a> {
    fn runtime(&self, kind: &'a str, command: &'a str) -> ProcessRuntime<'a> {
        ProcessRuntime {
            app: self.app,
            app_path: self.app_path,
            kind,
            command,
            env: self.env,
            paths: self.paths,
        }
    }
}

/// Publishes a Procfile app's backends to the active router.
///
/// The app's ENV file is read on every publish: starting a copy persists the
/// routing keys (`RIKU_NGINX_WSGI`, `RIKU_NGINX_PORTMAP`) there, and the
/// router picks its template from them.
pub(super) struct WebRoutes<'a> {
    pub app: &'a str,
    pub app_path: &'a Path,
    pub paths: &'a RikuPaths,
}

impl RouteSink for WebRoutes<'_> {
    fn publish(&mut self, backends: &[Backend]) -> Result<()> {
        let env = match crate::supervisor_ctl::read_app_env(self.app, self.paths) {
            Ok(env) => env,
            Err(e) => return Err(e),
        };
        crate::router::apply(self.app, self.app_path, &env, self.paths, backends)
    }

    fn check(&mut self) -> Result<()> {
        let env = match crate::supervisor_ctl::read_app_env(self.app, self.paths) {
            Ok(env) => env,
            Err(e) => return Err(e),
        };
        crate::router::check(self.app, &env, self.paths)
    }
}

/// Copies overlap unless two of them would need the same address: a pinned
/// `PORT` for `web`, or a shared `RIKU_NGINX_SOCKET` for a wsgi kind. Those
/// stop the old copy before its replacement starts.
fn web_order(kind: &str, env: &HashMap<String, String>) -> Result<ReplaceOrder> {
    if kind == WEB_KIND {
        match pinned_port(env) {
            Ok(Some(_)) => return Ok(ReplaceOrder::StopFirst),
            Ok(None) => {}
            Err(e) => return Err(e),
        }
    }
    if is_wsgi_kind(kind) && env.contains_key("RIKU_NGINX_SOCKET") {
        return Ok(ReplaceOrder::StopFirst);
    }
    Ok(ReplaceOrder::Overlap)
}

/// Replaces every copy of `kind` and leaves `want` of them running.
pub(super) fn roll_web_kind(
    deploy: &WebDeploy<'_>,
    kind: &str,
    command: &str,
    want: u32,
    routes: &mut impl RouteSink,
) -> Result<()> {
    let order = match web_order(kind, deploy.env) {
        Ok(order) => order,
        Err(e) => return Err(e),
    };
    let mut runtime = deploy.runtime(kind, command);
    match replace_all(&mut runtime, routes, order, want) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!("could not replace the {kind} copies"))),
    }
}

/// Stops every copy of `kind`, for a kind scaled to 0 or gone from the
/// Procfile. The router is not touched here: the deploy reconfigures it from
/// the copies left enabled.
pub(super) fn stop_web_kind(deploy: &WebDeploy<'_>, kind: &str) -> Result<()> {
    let mut runtime = deploy.runtime(kind, "");
    let live = match runtime.live() {
        Ok(live) => live,
        Err(e) => return Err(e.context(format!("could not list the {kind} copies"))),
    };
    for copy in &live {
        if let Err(e) = runtime.stop(copy) {
            return Err(e.context(format!("could not stop {kind} copy {}", copy.copy)));
        }
    }
    Ok(())
}

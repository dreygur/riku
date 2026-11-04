//! Where a rolling replacement of container copies publishes its backends.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use riku_util::backend::Backend;

use crate::config::RikuPaths;
use crate::rolling::RouteSink;

/// The route sink for one service of a container app.
pub(crate) enum ServiceRoutes<'a> {
    /// The service that serves the app's domains: every publish rewrites the
    /// router config and waits for the confirmed reload.
    Router {
        app: &'a str,
        app_path: &'a Path,
        env: &'a HashMap<String, String>,
        paths: &'a RikuPaths,
    },
    /// Any other service. Nothing is routed to it.
    Unrouted,
}

impl RouteSink for ServiceRoutes<'_> {
    fn publish(&mut self, backends: &[Backend]) -> Result<()> {
        match self {
            ServiceRoutes::Router {
                app,
                app_path,
                env,
                paths,
            } => crate::router::apply(app, app_path, env, paths, backends),
            ServiceRoutes::Unrouted => Ok(()),
        }
    }

    fn check(&mut self) -> Result<()> {
        match self {
            ServiceRoutes::Router {
                app,
                app_path,
                env,
                paths,
            } => crate::router::check(app, app_path, env, paths),
            ServiceRoutes::Unrouted => Ok(()),
        }
    }
}

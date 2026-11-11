//! Writing the copies of a kind no router fronts, and stopping web facing
//! kinds the Procfile dropped.

use anyhow::Result;
use std::collections::HashMap;

use riku_container::types::CopyIndex;

use super::files::write_worker_config;
use super::web_env::build_worker_env;
use super::web_rollout::{stop_web_kind, WebDeploy};

/// Writes `count` configs for a kind no router fronts. Its old copies were
/// removed first (see `remove_stale_configs`), so this is stop first.
pub(super) fn write_copies(
    deploy: &WebDeploy<'_>,
    kind: &str,
    command: &str,
    count: u32,
) -> Result<()> {
    for copy in (1..=count).filter_map(CopyIndex::new) {
        let worker_env =
            match build_worker_env(deploy.app, kind, copy, deploy.env, &[], deploy.paths) {
                Ok(worker_env) => worker_env,
                Err(e) => return Err(e),
            };
        let written = write_worker_config(
            deploy.app,
            deploy.app_path,
            kind,
            command,
            copy.get(),
            worker_env,
            deploy.paths,
        );
        if let Err(e) = written {
            return Err(e);
        }
    }
    Ok(())
}

/// Stops the web facing kinds that still run but left the Procfile, such as
/// `wsgi` after the app moved to `web`.
pub(super) fn stop_dropped_kinds(
    deploy: &WebDeploy<'_>,
    kept: &HashMap<String, String>,
) -> Result<()> {
    let running = match crate::router::web_copies(deploy.app, deploy.paths) {
        Ok(running) => running,
        Err(e) => return Err(e),
    };
    let mut dropped: Vec<&str> = running
        .iter()
        .map(|config| config.worker.kind.as_str())
        .filter(|kind| !kept.contains_key(*kind))
        .collect();
    dropped.sort_unstable();
    dropped.dedup();
    for kind in dropped {
        if let Err(e) = stop_web_kind(deploy, kind) {
            return Err(e);
        }
    }
    Ok(())
}

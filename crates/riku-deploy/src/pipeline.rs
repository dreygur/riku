//! The steps of one deploy, in order: sync the checkout, load the config,
//! then a container deploy or a Procfile deploy, then the finish. Each step
//! reports through [`DeployProgress`], which also reports a failure.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use std::fs;

use anyhow::Result;

use crate::config::RikuPaths;
use crate::progress::{DeployProgress, Phase};
use crate::{
    containers, deploy_env, deploy_finish, env_setup, git_ops, hooks, procfile_deploy, workers,
};

/// Pre deploy hooks, then the container deploy and the app's `LIVE_ENV`.
/// Copy counts of a container app change through `scale_service`, never
/// through deploy deltas.
fn deploy_container_app(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    env: &HashMap<String, String>,
    progress: &mut DeployProgress,
) -> Result<()> {
    if !deltas.is_empty() {
        return Err(anyhow::anyhow!(
            "'{app}' runs as containers; scale it with riku ps {app} --scale <service>=<copies>"
        ));
    }
    let name = match containers::AppName::try_from(app) {
        Ok(name) => name,
        Err(e) => return Err(e),
    };
    progress.log.log_raw("Running pre-deploy hooks");
    if let Err(e) = hooks::run_pre_deploy(app, &paths.app_root.join(app), paths, env) {
        return Err(e);
    }
    progress.begin(Phase::Runtime);
    progress.done(containers::CONTAINER_RUNTIME);
    progress.begin(Phase::Processes);
    let replaced =
        containers::deploy_containers(&name, paths, env, || progress.release_replacing());
    if let Err(e) = replaced {
        return Err(e);
    }
    if let Err(e) = env_setup::write_live_env(app, paths, env) {
        return Err(e);
    }
    progress.done_timed("containers");
    Ok(())
}

/// Sync the checkout to `newrev` and print the intro naming its commit.
fn sync(
    app_path: &std::path::Path,
    newrev: Option<&str>,
    progress: &mut DeployProgress,
) -> Result<()> {
    progress.begin(Phase::Sync);
    progress
        .log
        .log_raw(&format!("Syncing repo to {}", newrev.unwrap_or("HEAD")));
    let synced = git_ops::sync_app_repo(app_path, newrev);
    let commit = match &synced {
        Ok(()) => git_ops::head_commit(app_path),
        Err(_) => None,
    };
    progress.intro(commit.as_ref());
    synced
}

/// Everything [`crate::do_deploy`] does once it holds the deploy lock.
pub(crate) fn run(
    app: &str,
    paths: &RikuPaths,
    deltas: &HashMap<String, i64>,
    newrev: Option<&str>,
    progress: &mut DeployProgress,
) -> Result<()> {
    let app_path = paths.app_root.join(app);
    if let Err(e) = sync(&app_path, newrev, progress) {
        return Err(e);
    }

    progress.begin(Phase::Config);
    if let Err(e) = fs::create_dir_all(paths.log_root.join(app)) {
        return Err(anyhow::anyhow!(
            "cannot create the log directory of '{app}': {e}"
        ));
    }
    let mut env = match deploy_env::load_deploy_env(app, paths, &mut progress.log) {
        Ok(env) => env,
        Err(e) => return Err(e),
    };
    let is_container = match containers::is_container_app(&app_path, &env) {
        Ok(is_container) => is_container,
        Err(e) => return Err(e),
    };

    let deployed = if is_container {
        if let Err(e) = deploy_env::refuse_container_processes(app, paths) {
            return Err(e);
        }
        // Keys a former Procfile web kind left would pick the port map or
        // uWSGI template for the containers.
        workers::web_env::drop_route_keys(&mut env);
        if let Err(e) = workers::web_env::clear_route_env(app, paths) {
            return Err(e);
        }
        match deploy_container_app(app, paths, deltas, &env, progress) {
            Ok(()) => procfile_deploy::Deployed {
                runtime: containers::CONTAINER_RUNTIME.to_string(),
                workers: String::new(),
            },
            Err(e) => return Err(e),
        }
    } else {
        let processes = match deploy_env::load_deploy_processes(app, paths) {
            Ok(processes) => processes,
            Err(e) => return Err(e),
        };
        let deploy = procfile_deploy::deploy_procfile_app(
            app,
            paths,
            deltas,
            processes.unwrap_or_default(),
            &mut env,
            progress,
        );
        match deploy {
            Ok(deployed) => deployed,
            Err(e) => return Err(e),
        }
    };

    deploy_finish::finish_deploy(app, paths, &deployed, &env, progress);
    Ok(())
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;

//! Everything one container app operation works with, loaded once.
//!
//! # Security model
//!
//! The scratch directory receives env files that hold app secrets for the
//! length of one engine `run`. It lives under the riku root with mode 0700,
//! and the engine driver writes each file with mode 0600 and removes it.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use riku_container::compose::{routed_http, ComposeProject, RoutedPort, ServiceDef};
use riku_container::engine::Engine;
use riku_container::types::ServiceName;

use super::app::{load_project, sorted_env};
use super::engine_open::open_engine;
use super::routes::ServiceRoutes;
use super::runtime::ContainerRuntime;
use super::AppName;
use crate::config::RikuPaths;

const SCRATCH_DIR: &str = "scratch";

fn scratch_dir(paths: &RikuPaths) -> Result<PathBuf> {
    let dir = paths.riku_root.join(SCRATCH_DIR);
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }
    match fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)) {
        Ok(()) => Ok(dir),
        Err(e) => Err(anyhow!("cannot restrict {}: {e}", dir.display())),
    }
}

/// The app's saved `ENV`, empty when it has none.
pub(crate) fn saved_env(app: &str, paths: &RikuPaths) -> Result<HashMap<String, String>> {
    let file = paths.env_root.join(app).join("ENV");
    let mut env = HashMap::new();
    match crate::util::parse_settings(&file, &mut env) {
        Ok(_) => Ok(env),
        Err(e) => Err(anyhow!("cannot read {}: {e}", file.display())),
    }
}

/// A container app with its engine, parsed project and routed port.
pub(crate) struct ContainerApp<'a> {
    pub app: &'a AppName,
    pub paths: &'a RikuPaths,
    pub app_path: PathBuf,
    pub env: HashMap<String, String>,
    pub container_env: BTreeMap<String, String>,
    pub project: ComposeProject,
    pub routed: Option<RoutedPort>,
    pub engine: Engine,
    scratch: PathBuf,
}

impl<'a> ContainerApp<'a> {
    pub fn load(
        app: &'a AppName,
        paths: &'a RikuPaths,
        env: HashMap<String, String>,
    ) -> Result<ContainerApp<'a>> {
        let app_path = paths.app_root.join(app.as_str());
        let project = match load_project(app.as_str(), &app_path, &env) {
            Ok(project) => project,
            Err(e) => return Err(e.context(format!("cannot read the services of '{app}'"))),
        };
        let routed = match routed_http(&project) {
            Ok(routed) => routed,
            Err(e) => return Err(e),
        };
        let engine = match open_engine(paths) {
            Ok(engine) => engine,
            Err(e) => return Err(e),
        };
        let scratch = match scratch_dir(paths) {
            Ok(dir) => dir,
            Err(e) => return Err(e),
        };
        Ok(ContainerApp {
            app,
            paths,
            container_env: sorted_env(&env),
            app_path,
            env,
            project,
            routed,
            engine,
            scratch,
        })
    }

    pub fn service(&self, name: &ServiceName) -> Result<&ServiceDef> {
        match self.project.services.iter().find(|s| &s.name == name) {
            Some(service) => Ok(service),
            None => Err(anyhow!(
                "app '{}' has no service '{name}' in its compose file",
                self.app
            )),
        }
    }

    /// The 0700 directory engine env files are written to.
    pub fn scratch(&self) -> &std::path::Path {
        &self.scratch
    }

    pub fn is_routed(&self, service: &ServiceName) -> bool {
        self.routed.as_ref().is_some_and(|r| &r.service == service)
    }

    pub fn runtime(&'a self, service: &'a ServiceDef) -> ContainerRuntime<'a> {
        ContainerRuntime {
            engine: &self.engine,
            app: self.app,
            project: &self.project,
            service,
            app_path: &self.app_path,
            scratch: &self.scratch,
            env: &self.container_env,
            routed: self.is_routed(&service.name),
        }
    }

    pub fn routes(&'a self, service: &ServiceName) -> ServiceRoutes<'a> {
        if !self.is_routed(service) {
            return ServiceRoutes::Unrouted;
        }
        ServiceRoutes::Router {
            app: self.app.as_str(),
            app_path: &self.app_path,
            env: &self.env,
            paths: self.paths,
        }
    }
}

//! A whole compose project: services in start order and declared volumes.
//!
//! # Security model
//!
//! The compose file is attacker controlled app code and engine volume names
//! are shared by every app on the server. Volumes are therefore always named
//! `<app>.<key>` from the riku app name and a validated key; the compose file
//! cannot pick a volume name itself (see `volume_decl`), and its `name` and
//! `COMPOSE_PROJECT_NAME` play no part.

use super::order::start_order;
use super::volume_decl::parse_volume_decls;
use super::{parse_service, read_raw, BuildDef, ComposeFile, ServiceContext, ServiceDef};
use crate::types::{RestartPolicy, ServiceName, StopSignal, DEFAULT_STOP_GRACE};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// How a top level volume is named in the engine. The compose file cannot
/// choose an engine name, so there is one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeDecl {
    /// Named `<app>.<key>`.
    Managed,
}

/// The validated contents of a compose project.
#[derive(Debug)]
pub struct ComposeProject {
    /// Services in start order: dependencies first, ties by name.
    pub services: Vec<ServiceDef>,
    /// Non fatal findings, such as unset variables.
    pub warnings: Vec<String>,
    volumes: BTreeMap<String, VolumeDecl>,
}

impl ComposeProject {
    /// Declared top level volumes by key.
    pub fn volumes(&self) -> &BTreeMap<String, VolumeDecl> {
        &self.volumes
    }
}

/// Read and check the compose file at `compose` inside `app_path`.
///
/// Variables come from the `.env` next to the compose file overlaid by
/// `app_env`. Services come back in start order.
pub fn read_project(
    app_path: &Path,
    compose: &ComposeFile,
    app_env: &BTreeMap<String, String>,
) -> Result<ComposeProject> {
    let raw = match read_raw(app_path, compose, app_env) {
        Ok(raw) => raw,
        Err(e) => return Err(e),
    };
    let compose_dir = app_path
        .join(compose.relative())
        .parent()
        .unwrap_or(app_path)
        .to_path_buf();
    let volumes = match parse_volume_decls(&raw.volumes) {
        Ok(volumes) => volumes,
        Err(e) => return Err(e),
    };
    let declared_volumes: BTreeSet<String> = volumes.keys().cloned().collect();
    let ctx = ServiceContext {
        app_path,
        compose_dir: &compose_dir,
        vars: &raw.vars,
        declared_volumes: &declared_volumes,
    };
    let mut services = Vec::with_capacity(raw.services.len());
    for (name, value) in &raw.services {
        match parse_service(&ctx, name, value) {
            Ok(service) => services.push(service),
            Err(e) => return Err(e),
        }
    }
    let services = match start_order(services) {
        Ok(services) => services,
        Err(e) => return Err(e),
    };
    Ok(ComposeProject {
        services,
        volumes,
        warnings: raw.warnings,
    })
}

/// The project for an app that has a Dockerfile and no compose file: one
/// service `app` built from the app directory, publishing no ports and
/// restarted by the supervisor unless the user stopped it.
pub fn single_image_project(app_path: &Path, dockerfile: &Path) -> Result<ComposeProject> {
    let name = ServiceName::try_from("app").expect("'app' is a valid service name");
    let service = ServiceDef {
        name,
        image: None,
        build: Some(BuildDef {
            context: app_path.to_path_buf(),
            dockerfile: Some(dockerfile.to_path_buf()),
            args: BTreeMap::new(),
        }),
        command: None,
        entrypoint: None,
        environment: BTreeMap::new(),
        ports: Vec::new(),
        mounts: Vec::new(),
        depends_on: Vec::new(),
        healthcheck: None,
        restart: RestartPolicy::UnlessStopped,
        stop_grace: DEFAULT_STOP_GRACE,
        stop_signal: StopSignal::default(),
        labels: BTreeMap::new(),
        working_dir: None,
        user: None,
    };
    Ok(ComposeProject {
        services: vec![service],
        volumes: BTreeMap::new(),
        warnings: Vec::new(),
    })
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;

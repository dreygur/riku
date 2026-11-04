//! A whole compose project: services in start order, volumes and project name.
//!
//! # Security model
//!
//! The compose file is attacker controlled app code and engine volume names
//! are shared by every app on the server. Volumes are therefore always named
//! `<project>_<key>` from a validated project name and a validated key; the
//! compose file cannot pick a volume name itself (see `volume_decl`).
//! The remaining way to reach another app's volumes is the project name, which
//! is why [`ProjectNameSource`] exists.

use super::order::start_order;
use super::project_name::{project_name_for_app, resolve_project_name};
use super::volume_decl::parse_volume_decls;
use super::{parse_service, read_raw, BuildDef, ComposeFile, ServiceContext, ServiceDef};
use crate::types::{RestartPolicy, ServiceName, StopSignal, DEFAULT_STOP_GRACE};
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// How a top level volume is named in the engine. The compose file cannot
/// choose an engine name, so there is one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeDecl {
    /// Named `<project>_<key>`.
    Managed,
}

/// Where the project name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectNameSource {
    /// The app's own name, unchanged by normalisation: taken from the app
    /// directory holding the compose file, or from the app name of a
    /// Dockerfile app. An app whose name normalisation changes (`Shop`,
    /// `sh.op`) gets `Declared` instead, since several apps share the result.
    Derived,
    /// Chosen by the compose file (`name`) or the app env
    /// (`COMPOSE_PROJECT_NAME`), so it can be any name, including another
    /// riku app's. Volume names are not produced for it.
    Declared,
    /// A `Declared` name that [`ComposeProject::confirm_declared_name`] has
    /// accepted.
    DeclaredConfirmed,
}

/// The validated contents of a compose project.
///
/// The fields that decide volume names are private and read through getters,
/// so nothing outside this module can mark a name `Derived`.
///
/// A `Declared` project name is only safe once the state aware migration step
/// has refused one that equals another riku app's name, or whose existing
/// volumes lack the matching compose project label. This crate has no server
/// state and cannot check that, so [`existing_volume_name`] returns an error
/// for managed volumes of a `Declared` project until that step calls
/// [`ComposeProject::confirm_declared_name`].
#[derive(Debug)]
pub struct ComposeProject {
    /// Services in start order: dependencies first, ties by name.
    pub services: Vec<ServiceDef>,
    /// Non fatal findings, such as unset variables.
    pub warnings: Vec<String>,
    volumes: BTreeMap<String, VolumeDecl>,
    project_name: String,
    name_source: ProjectNameSource,
}

impl ComposeProject {
    /// Declared top level volumes by key.
    pub fn volumes(&self) -> &BTreeMap<String, VolumeDecl> {
        &self.volumes
    }

    /// Compose project name, lowercase and engine safe. It prefixes managed
    /// volume names.
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    /// Where the project name came from.
    pub fn name_source(&self) -> ProjectNameSource {
        self.name_source
    }

    /// Record that the migration step checked the declared project name
    /// against the other apps and their volumes. Does nothing for a derived
    /// name.
    pub fn confirm_declared_name(&mut self) {
        if self.name_source == ProjectNameSource::Declared {
            self.name_source = ProjectNameSource::DeclaredConfirmed;
        }
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
    let (project_name, name_source) =
        match resolve_project_name(&raw.vars, raw.name.as_deref(), app_path, &compose_dir) {
            Ok(resolved) => resolved,
            Err(e) => return Err(e),
        };
    Ok(ComposeProject {
        services,
        volumes,
        project_name,
        name_source,
        warnings: raw.warnings,
    })
}

/// The project for the riku app `app` when it has a Dockerfile and no compose
/// file: one service `app` built from the app directory, publishing no ports
/// and restarted by the supervisor unless the user stopped it.
/// The project name is the app's name normalised; it is `Derived` only when
/// normalising left the name unchanged, since `Shop` and `shop` share `shop`.
pub fn single_image_project(
    app: &str,
    app_path: &Path,
    dockerfile: &Path,
) -> Result<ComposeProject> {
    let (project_name, name_source) = match project_name_for_app(app) {
        Ok(resolved) => resolved,
        Err(e) => return Err(e),
    };
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
        project_name,
        name_source,
        warnings: Vec::new(),
    })
}

/// The engine name an existing compose project gave the volume `key`, so a
/// migrated app finds its data. `Ok(None)` for a key the project does not
/// declare.
///
/// Fails for a managed volume of a project whose name is still `Declared`.
pub fn existing_volume_name(project: &ComposeProject, key: &str) -> Result<Option<String>> {
    let Some(decl) = project.volumes.get(key) else {
        return Ok(None);
    };
    match decl {
        VolumeDecl::Managed if project.name_source == ProjectNameSource::Declared => Err(anyhow!(
            "volume '{key}' would be named after the declared project name '{}', which is not confirmed as this app's own",
            project.project_name
        )),
        VolumeDecl::Managed => Ok(Some(format!("{}_{key}", project.project_name))),
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;

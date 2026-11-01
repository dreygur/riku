//! Telling a container app apart and loading what its deploy needs: the
//! engine, the compose project and the wanted copy counts.
//!
//! # Security model
//!
//! The compose file, `COMPOSE_FILE` and `SCALING` are app owner input. The
//! compose parser refuses paths outside the checkout and unknown keys; copy
//! counts that are not numbers are refused here. The engine program comes
//! from the operator's `CONTAINER_ENGINE` setting or a fixed list searched on
//! `PATH`, never from app input.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_container::compose::ComposeProject;
use riku_container::compose::{find_compose_file, read_project, single_image_project};
use riku_container::engine::{detect, Engine};
use riku_container::types::{HostIp, PortBinding, Protocol, ServiceName};

use crate::config::RikuPaths;

/// The port a single Dockerfile app listens on inside its container. It is
/// also passed as `PORT`, so apps that read `$PORT` keep working.
pub const SINGLE_IMAGE_PORT: u16 = 8000;

const RUNTIME_KEY: &str = "RIKU_RUNTIME";
const RUNTIME_NAME: &str = "container";
const COMPOSE_FILE_KEY: &str = "COMPOSE_FILE";
const IMAGE_FILES: [&str; 2] = ["Containerfile", "Dockerfile"];

fn image_file(app_path: &Path) -> Option<PathBuf> {
    IMAGE_FILES
        .iter()
        .map(|name| app_path.join(name))
        .find(|path| path.is_file())
}

fn requested_compose(env: &HashMap<String, String>) -> Option<&str> {
    env.get(COMPOSE_FILE_KEY)
        .map(String::as_str)
        .filter(|name| !name.is_empty())
}

/// Whether riku runs `app_path` as containers: it has a compose file (or a
/// `COMPOSE_FILE` that must then be valid) or a `Containerfile` or
/// `Dockerfile` at its root, and `RIKU_RUNTIME` is unset or `container`.
pub fn is_container_app(app_path: &Path, env: &HashMap<String, String>) -> Result<bool> {
    if env
        .get(RUNTIME_KEY)
        .is_some_and(|name| name != RUNTIME_NAME)
    {
        return Ok(false);
    }
    match find_compose_file(app_path, requested_compose(env)) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(image_file(app_path).is_some()),
        Err(e) => Err(e),
    }
}

/// Open the engine the server is set up to use.
pub fn open_engine(paths: &RikuPaths) -> Result<Engine> {
    let setting = match riku_config::container_engine::load(paths) {
        Ok(setting) => setting,
        Err(e) => return Err(e),
    };
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let binary = match detect(setting.as_deref(), &search_path) {
        Ok(binary) => binary,
        Err(e) => return Err(e),
    };
    Engine::open(binary)
}

/// The app env as the compose reader and the run spec want it.
pub(crate) fn sorted_env(env: &HashMap<String, String>) -> BTreeMap<String, String> {
    env.iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn single_image(app: &str, app_path: &Path, dockerfile: &Path) -> Result<ComposeProject> {
    let mut project = match single_image_project(app, app_path, dockerfile) {
        Ok(project) => project,
        Err(e) => return Err(e),
    };
    let service = &mut project.services[0];
    service.ports.push(PortBinding {
        host_ip: HostIp::Loopback,
        host_port: None,
        container_port: SINGLE_IMAGE_PORT,
        protocol: Protocol::Tcp,
    });
    service
        .environment
        .insert("PORT".to_string(), SINGLE_IMAGE_PORT.to_string());
    Ok(project)
}

/// Read the app's compose project, or build the one service project of a
/// single Dockerfile app.
pub fn load_project(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
) -> Result<ComposeProject> {
    let compose = match find_compose_file(app_path, requested_compose(env)) {
        Ok(compose) => compose,
        Err(e) => return Err(e),
    };
    if let Some(compose) = compose {
        return read_project(app_path, &compose, &sorted_env(env));
    }
    match image_file(app_path) {
        Some(dockerfile) => single_image(app, app_path, &dockerfile),
        None => Err(anyhow!(
            "app '{app}' has no compose file, Containerfile or Dockerfile"
        )),
    }
}

fn scaling_file(app: &str, paths: &RikuPaths) -> PathBuf {
    paths.env_root.join(app).join("SCALING")
}

fn parse_count(line: &str) -> Result<Option<(String, u32)>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (name, count) = match line.split_once('=') {
        Some((name, count)) if !name.trim().is_empty() => (name.trim(), count),
        Some(_) => return Err(anyhow!("SCALING line {line:?} has an empty service name")),
        None => return Err(anyhow!("SCALING line {line:?} is not <service>=<copies>")),
    };
    match count.trim().parse::<u32>() {
        Ok(count) => Ok(Some((name.to_string(), count))),
        Err(_) => Err(anyhow!("SCALING line {line:?} does not give a copy count")),
    }
}

/// Copy counts from the app's `SCALING` file, by service name. A missing
/// file means every service runs one copy.
pub fn read_scaling(app: &str, paths: &RikuPaths) -> Result<BTreeMap<String, u32>> {
    let path = scaling_file(app, paths);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        match parse_count(line) {
            Ok(Some((name, count))) => {
                counts.insert(name, count);
            }
            Ok(None) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(counts)
}

/// Record `copies` for `service` in the app's `SCALING` file.
pub(crate) fn write_scaling(
    app: &str,
    paths: &RikuPaths,
    service: &ServiceName,
    copies: u32,
) -> Result<()> {
    let mut counts = match read_scaling(app, paths) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };
    counts.insert(service.to_string(), copies);
    let text: String = counts
        .iter()
        .map(|(name, count)| format!("{name}={count}\n"))
        .collect();
    let path = scaling_file(app, paths);
    if let Err(e) = fs::create_dir_all(paths.env_root.join(app)) {
        return Err(anyhow!("cannot create the env directory of '{app}': {e}"));
    }
    match fs::write(&path, text) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", path.display())),
    }
}

/// The copies wanted for `service`: its `SCALING` entry, else 1.
pub(crate) fn wanted(counts: &BTreeMap<String, u32>, service: &ServiceName) -> u32 {
    counts.get(service.as_str()).copied().unwrap_or(1)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

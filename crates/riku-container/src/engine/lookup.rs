//! Finding containers and images by name, project or reference.
//!
//! Security model: container and project names pass the engine safe check
//! before they are placed in a `--filter` argument, and dots in a container
//! name are escaped because the engine reads the name filter as a regular
//! expression. Image references are refused when they could be read as a flag.

use std::collections::BTreeSet;
use std::ffi::OsString;

use anyhow::{anyhow, Result};

use super::spec_check::check_positional;
use super::{ContainerId, Engine};
use crate::types::{check_engine_name, labels, ContainerName};

fn exact_name_filter(name: &ContainerName) -> String {
    format!("name=^{}$", name.as_str().replace('.', r"\."))
}

impl Engine {
    /// The `riku.app` label of the container named exactly `name`.
    ///
    /// `None` when no such container exists, `Some("")` when it exists without
    /// the label, so the caller can tell a free name from one someone else holds.
    #[allow(clippy::question_mark)]
    pub fn owner_of(&self, name: &ContainerName) -> Result<Option<String>> {
        let ids = match self.list_with_filter(Some(&exact_name_filter(name))) {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };
        let infos = match self.inspect(&ids) {
            Ok(infos) => infos,
            Err(e) => return Err(e),
        };
        let owner = infos
            .into_iter()
            .find(|info| info.name == name.as_str())
            .map(|info| info.app.unwrap_or_default());
        Ok(owner)
    }

    /// Full ids of every container of a compose project, running or not.
    pub fn list_project(&self, project: &str) -> Result<Vec<ContainerId>> {
        match check_engine_name("project name", project) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        self.list_with_filter(Some(&format!(
            "label={}={project}",
            labels::COMPOSE_PROJECT
        )))
    }

    /// The engine's id for a local image, or `None` when it is not present.
    ///
    /// A reference that matches several images, such as a repository without a
    /// tag, is an error.
    pub fn image_id(&self, image: &str) -> Result<Option<String>> {
        match check_positional("image", image) {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
        let args: Vec<OsString> = ["image", "ls", "-q", "--no-trunc", image]
            .into_iter()
            .map(OsString::from)
            .collect();
        let listing = match self.output(&args) {
            Ok(listing) => listing,
            Err(e) => return Err(anyhow!("cannot look up image {image:?}: {e}")),
        };
        let ids: BTreeSet<&str> = listing
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        match ids.len() {
            0 => Ok(None),
            1 => Ok(ids.into_iter().next().map(str::to_string)),
            _ => Err(anyhow!(
                "image {image:?} matches several images; name one with a tag"
            )),
        }
    }
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;

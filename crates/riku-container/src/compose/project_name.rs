//! Which project name docker compose used for an app's volumes.
//!
//! The compose specification says "Project names must contain only lowercase
//! letters, decimal digits, dashes, and underscores, and must begin with a
//! lowercase letter or decimal digit." The top level `name` key is held to
//! that text as written. `COMPOSE_PROJECT_NAME` and the directory name are
//! normalised the way docker compose does it (lowercase, other characters
//! dropped); that normalisation is docker compose behaviour and is not spelled
//! out in the specification text.
//!
//! # Security model
//!
//! The project name prefixes every engine volume name, so it decides which
//! volumes an app can reach. Every name is checked with the engine safe
//! character set before it is stored, and a name taken from the compose file
//! or the app env, or from a subdirectory of the app, is reported as
//! `Declared`, never as `Derived`.

use super::project::ProjectNameSource;
use crate::types::is_engine_safe;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::Path;

const ENV_KEY: &str = "COMPOSE_PROJECT_NAME";

/// Pick the project name: `COMPOSE_PROJECT_NAME` in `vars` (the project `.env`
/// overlaid by the app env), else the compose file's top level `name`, else
/// the name of `compose_dir`.
///
/// Only a directory name taken from `app_path` itself is `Derived`. A name
/// taken from a subdirectory can collide with another app's, so it is
/// `Declared` like the other sources.
pub(super) fn resolve_project_name(
    vars: &BTreeMap<String, String>,
    declared: Option<&str>,
    app_path: &Path,
    compose_dir: &Path,
) -> Result<(String, ProjectNameSource)> {
    if let Some(raw) = vars.get(ENV_KEY).filter(|raw| !raw.is_empty()) {
        return checked(normalise(raw), raw, ENV_KEY).map(|n| (n, ProjectNameSource::Declared));
    }
    if let Some(raw) = declared {
        let name = raw.to_string();
        if normalise(raw) != name {
            bail!("top level name {raw:?} is not a valid project name: use lowercase letters, digits, '-' and '_'");
        }
        return checked(name, raw, "top level name").map(|n| (n, ProjectNameSource::Declared));
    }
    let dir = compose_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let source = if compose_dir == app_path {
        ProjectNameSource::Derived
    } else {
        ProjectNameSource::Declared
    };
    checked(normalise(&dir), &dir, "compose directory name").map(|n| (n, source))
}

fn normalise(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .filter(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'))
        .collect()
}

fn checked(name: String, raw: &str, origin: &str) -> Result<String> {
    if !is_engine_safe(&name) {
        bail!("{origin} {raw:?} does not give a valid project name");
    }
    Ok(name)
}

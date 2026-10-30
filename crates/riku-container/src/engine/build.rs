//! Building an image from a directory.
//!
//! Security model: the context must be an absolute path, so it cannot be read
//! as a flag and does not depend on riku's working directory. The engine
//! resolves a relative `-f` against that working directory, not the context,
//! so the Dockerfile is always passed as an absolute path under the context
//! and one with a `..` component, or outside the context, is refused.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Result};

use super::env_file::is_valid_name;
use super::Engine;

/// An image to build from a directory.
///
/// Build arguments are passed on the command line, so they are visible in the
/// process list while the build runs. Do not put secrets in them.
#[derive(Debug, Clone)]
pub struct BuildSpec {
    /// Absolute path of the build context directory.
    pub context: PathBuf,
    /// Dockerfile path, relative to the context or absolute inside it. `None`
    /// uses the engine's default in the context.
    pub dockerfile: Option<PathBuf>,
    pub tag: String,
    /// Argument names must be non-empty and hold no `=`, whitespace or control characters.
    pub args: BTreeMap<String, String>,
    pub platform: Option<String>,
}

fn dockerfile_path(context: &Path, dockerfile: &Path) -> Result<PathBuf> {
    let climbs = dockerfile
        .components()
        .any(|c| matches!(c, Component::ParentDir));
    let joined = context.join(dockerfile);
    if climbs || !joined.starts_with(context) {
        return Err(anyhow!(
            "dockerfile {} must be inside the build context {}",
            dockerfile.display(),
            context.display()
        ));
    }
    Ok(joined)
}

fn build_args(spec: &BuildSpec) -> Result<Vec<OsString>> {
    if !spec.context.is_absolute() {
        return Err(anyhow!(
            "build context {} must be an absolute path",
            spec.context.display()
        ));
    }
    if let Some(key) = spec.args.keys().find(|k| !is_valid_name(k)) {
        return Err(anyhow!(
            "build argument name {key:?} is not allowed: it must be non-empty and hold no '=', whitespace or control characters"
        ));
    }
    let mut args: Vec<OsString> = vec!["build".into(), "-t".into(), (&spec.tag).into()];
    if let Some(dockerfile) = &spec.dockerfile {
        match dockerfile_path(&spec.context, dockerfile) {
            Ok(path) => {
                args.push("-f".into());
                args.push(path.into());
            }
            Err(e) => return Err(anyhow!("invalid dockerfile: {e}")),
        }
    }
    for (key, value) in &spec.args {
        args.push("--build-arg".into());
        args.push(format!("{key}={value}").into());
    }
    if let Some(platform) = &spec.platform {
        args.push("--platform".into());
        args.push(platform.into());
    }
    args.push((&spec.context).into());
    Ok(args)
}

impl Engine {
    /// Build an image from `spec.context`.
    pub fn build(&self, spec: &BuildSpec) -> Result<()> {
        let args = match build_args(spec) {
            Ok(args) => args,
            Err(e) => return Err(anyhow!("cannot build {}: {e}", spec.tag)),
        };
        self.output(&args).map(|_| ())
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;

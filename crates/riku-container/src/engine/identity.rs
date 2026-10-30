//! The engine handle and what it reports about itself.
//!
//! Security model: the program path comes from [`super::detect`], which has
//! already refused relative and whitespace-bearing settings. Arguments are
//! passed as separate argv elements, never through a shell.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};
use riku_util::spawn::output_retrying_etxtbsy;
use serde_json::Value;

use super::detect::EngineBinary;
use super::probe::{first_unanswered, REQUIRED_COMMANDS};

/// Product name and version an engine reports, for display only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineIdentity {
    /// The engine's own product name, or the binary name when it reports none.
    pub name: String,
    /// Server version when a server answered, else the client version.
    pub version: String,
}

/// Read the identity out of `version --format json` output.
pub fn parse_identity(binary_name: &str, version_json: &str) -> Result<EngineIdentity> {
    let doc: Value = match serde_json::from_str(version_json) {
        Ok(doc) => doc,
        Err(e) => return Err(anyhow!("engine version output is not JSON: {e}")),
    };
    let name = match doc["Server"]["Platform"]["Name"].as_str() {
        Some(name) if !name.is_empty() => name,
        _ => binary_name,
    };
    let version = match doc["Server"]["Version"]
        .as_str()
        .or_else(|| doc["Client"]["Version"].as_str())
    {
        Some(version) => version,
        None => return Err(anyhow!("engine version output has no version")),
    };
    Ok(EngineIdentity {
        name: name.to_string(),
        version: version.to_string(),
    })
}

/// A container engine that passed the command probe.
#[derive(Debug, Clone)]
pub struct Engine {
    program: PathBuf,
    identity: EngineIdentity,
}

impl Engine {
    /// Probe the engine's commands, then read its identity.
    pub fn open(binary: EngineBinary) -> Result<Engine> {
        if let Some((command, reason)) = first_unanswered(&binary.program) {
            return Err(anyhow!(
                "container engine {} does not answer '{command}'; riku needs: {} ({reason})",
                binary.program.display(),
                REQUIRED_COMMANDS.join(", ")
            ));
        }
        let mut engine = Engine {
            program: binary.program,
            identity: EngineIdentity {
                name: binary.binary_name.clone(),
                version: String::new(),
            },
        };
        let json = match engine.output(&["version".into(), "--format".into(), "json".into()]) {
            Ok(json) => json,
            Err(e) => return Err(anyhow!("cannot read the engine version: {e}")),
        };
        engine.identity = match parse_identity(&binary.binary_name, &json) {
            Ok(identity) => identity,
            Err(e) => {
                return Err(anyhow!(
                    "container engine {}: {e}",
                    engine.program.display()
                ))
            }
        };
        Ok(engine)
    }

    /// What the engine reported about itself.
    pub fn identity(&self) -> &EngineIdentity {
        &self.identity
    }

    #[allow(dead_code)]
    pub(crate) fn program(&self) -> &Path {
        &self.program
    }

    /// Run the engine with `args` and return stdout.
    ///
    /// A non zero exit is an error carrying the command name and trimmed stderr.
    pub(crate) fn output(&self, args: &[OsString]) -> Result<String> {
        let command = args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        let out = match output_retrying_etxtbsy(Command::new(&self.program).args(args)) {
            Ok(out) => out,
            Err(e) => {
                return Err(anyhow!(
                    "cannot run '{command}' on the container engine: {e}"
                ))
            }
        };
        if !out.status.success() {
            return Err(anyhow!(
                "container engine '{command}' failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;

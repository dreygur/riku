//! The environment variables every engine call runs with.
//!
//! sshd, the supervisor and init each start riku with a different
//! environment, so the engine client would find a different daemon depending
//! on who ran it. The variables here are computed from the running uid and
//! `settings.toml` only, and [`apply_env`] drops whatever the caller had set
//! under the same names before adding them.
//!
//! Security model: the deploy user can edit `settings.toml`, so `host` and
//! `mode` are treated as input. Switching to rootful gains the deploy user
//! nothing unless it can already reach a rootful socket. In rootless mode the
//! runtime directory must be a real directory (not a symlink) owned by the
//! uid with mode 0700, so another local user cannot plant a socket the engine
//! client would then talk to.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use riku_config::settings::EngineMode;

const RUN_USER_ROOT: &str = "/run/user";
const RUNTIME_DIR_MODE: u32 = 0o700;
const BUS_SOCKET: &str = "bus";

const RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";
const SESSION_BUS: &str = "DBUS_SESSION_BUS_ADDRESS";
const ENGINE_HOST: &str = "DOCKER_HOST";

// Each of these can point the client at another daemon. Riku never sets them.
const CALLER_ONLY: [&str; 3] = ["DOCKER_CONTEXT", "CONTAINER_HOST", "CONTAINER_CONNECTION"];

fn check_runtime_dir(dir: &Path, uid: u32) -> Result<()> {
    let meta = match fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(e) => {
            return Err(anyhow!(
                "rootless engine needs {}, which cannot be read: {e}",
                dir.display()
            ))
        }
    };
    if !meta.is_dir() {
        return Err(anyhow!(
            "rootless engine needs {} to be a directory",
            dir.display()
        ));
    }
    if meta.uid() != uid {
        return Err(anyhow!(
            "{} is owned by uid {}, not uid {uid}",
            dir.display(),
            meta.uid()
        ));
    }
    let mode = meta.permissions().mode() & 0o7777;
    if mode != RUNTIME_DIR_MODE {
        return Err(anyhow!(
            "{} has mode {mode:04o}, it must be {RUNTIME_DIR_MODE:04o}",
            dir.display()
        ));
    }
    Ok(())
}

fn rootless_env(run_root: &Path, uid: u32) -> Result<Vec<(String, String)>> {
    let dir = run_root.join(uid.to_string());
    match check_runtime_dir(&dir, uid) {
        Ok(()) => {}
        Err(e) => return Err(e),
    }
    let mut env = vec![(RUNTIME_DIR.to_string(), dir.display().to_string())];
    let bus = dir.join(BUS_SOCKET);
    if bus.exists() {
        env.push((
            SESSION_BUS.to_string(),
            format!("unix:path={}", bus.display()),
        ));
    }
    Ok(env)
}

/// [`engine_env`] with the per user runtime directories under `run_root`
/// instead of `/run/user`.
pub(crate) fn engine_env_under(
    run_root: &Path,
    mode: EngineMode,
    host: Option<&str>,
    uid: u32,
) -> Result<Vec<(String, String)>> {
    let mut env = match mode {
        EngineMode::Rootless => match rootless_env(run_root, uid) {
            Ok(env) => env,
            Err(e) => return Err(anyhow!("rootless engine setup: {e}")),
        },
        EngineMode::Rootful => Vec::new(),
    };
    if let Some(host) = host {
        env.push((ENGINE_HOST.to_string(), host.to_string()));
    }
    Ok(env)
}

/// The variables to set on every engine call for `uid`.
///
/// Rootless sets `XDG_RUNTIME_DIR` to `/run/user/<uid>` and refuses unless
/// that directory exists, is owned by `uid` and has mode 0700. It adds
/// `DBUS_SESSION_BUS_ADDRESS` when the directory holds a `bus` socket. `host`
/// becomes `DOCKER_HOST` in either mode. Rootful sets nothing else.
pub fn engine_env(mode: EngineMode, host: Option<&str>, uid: u32) -> Result<Vec<(String, String)>> {
    engine_env_under(Path::new(RUN_USER_ROOT), mode, host, uid)
}

/// Set `env` on `command` after removing every variable that selects a
/// daemon, so the caller's environment cannot choose the engine.
pub(super) fn apply_env(command: &mut Command, env: &[(String, String)]) {
    for name in [RUNTIME_DIR, SESSION_BUS, ENGINE_HOST]
        .into_iter()
        .chain(CALLER_ONLY)
    {
        command.env_remove(name);
    }
    command.envs(env.iter().map(|(name, value)| (name, value)));
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;

//! The container part of `riku destroy`.

use anyhow::Result;

use super::control_target::{control_target, ControlTarget};
use super::destroy::DestroyMode;
use crate::config::RikuPaths;
use crate::deploy::containers::{destroy_containers, EngineUnreachable};
use crate::util::echo;

/// Stop and remove a container app's copies and network while its compose
/// file can still be read. Unlike plugin cleanup, a failure stops destroy:
/// running copies would keep their ports after riku forgot the app.
///
/// With the engine unreachable, `Safe` refuses with an error that is still an
/// [`EngineUnreachable`], and `Force` warns and lets destroy go on.
#[allow(clippy::question_mark)]
pub(super) fn container_cleanup(paths: &RikuPaths, app: &str, mode: DestroyMode) -> Result<()> {
    let target = match control_target(paths, app, None) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let container_app = match target {
        ControlTarget::Containers { app, .. } => app,
        ControlTarget::Procfile { .. } => return Ok(()),
    };
    let err = match destroy_containers(&container_app, paths) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    if err.downcast_ref::<EngineUnreachable>().is_none() {
        return Err(err);
    }
    match mode {
        DestroyMode::Safe => Err(err.context(format!(
            "containers of '{app}' would be orphaned, nothing was removed; \
             use --force to remove riku's files anyway"
        ))),
        DestroyMode::Force => {
            echo(
                &format!("Containers of '{app}' may still be running, remove them by hand"),
                "yellow",
            );
            Ok(())
        }
    }
}

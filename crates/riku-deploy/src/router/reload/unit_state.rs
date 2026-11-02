//! Reading the state of `riku-nginx-reload.service` through `systemctl show`.

use std::process::Command;

use anyhow::{anyhow, Result};

use super::nginx_unit::NGINX_UNIT;
use crate::config::ReloadPrograms;
use crate::util::spawn::output_retrying_etxtbsy;

const UNIT_PROPERTIES: &str = "ExecMainStartTimestampMonotonic,ExecMainExitTimestampMonotonic,ExecMainStatus,LoadState,ActiveState";

/// The reload unit's properties riku looks at.
pub(super) struct UnitState {
    pub(super) exit_ok: bool,
    pub(super) load_state: String,
    pub(super) active_state: String,
    pub(super) started_at: u64,
    pub(super) exited_at: u64,
    pub(super) status: String,
}

fn parse_timestamp(key: &str, raw: &str) -> u64 {
    if raw.is_empty() {
        return 0;
    }
    match raw.parse() {
        Ok(timestamp) => timestamp,
        Err(_) => {
            tracing::debug!("{NGINX_UNIT}: {key} is not a number, treating it as 0");
            0
        }
    }
}

pub(super) fn read_unit(programs: &ReloadPrograms) -> Result<UnitState> {
    let output = match output_retrying_etxtbsy(
        Command::new(&programs.systemctl)
            .args(["show", "-p", UNIT_PROPERTIES])
            .arg(NGINX_UNIT),
    ) {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("could not run systemctl: {e}")),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    Ok(UnitState {
        exit_ok: output.status.success(),
        load_state: value("LoadState"),
        active_state: value("ActiveState"),
        started_at: parse_timestamp(
            "ExecMainStartTimestampMonotonic",
            &value("ExecMainStartTimestampMonotonic"),
        ),
        exited_at: parse_timestamp(
            "ExecMainExitTimestampMonotonic",
            &value("ExecMainExitTimestampMonotonic"),
        ),
        status: value("ExecMainStatus"),
    })
}

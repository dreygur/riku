//! Restart policy parsing and backoff delay calculation.

use std::time::Duration;

use anyhow::{anyhow, Result};

const MAX_EXPONENT: u32 = 6;
const MAX_BACKOFF_SECS: u64 = 60;

fn parse_on_failure(raw: &str) -> Result<RestartPolicyLabel> {
    let remainder = match raw.strip_prefix("on-failure:") {
        Some(rest) => rest,
        None => return Err(anyhow!("invalid restart policy label")),
    };
    match remainder.parse::<u32>() {
        Ok(limit) => Ok(RestartPolicyLabel::OnFailure(Some(limit))),
        Err(_) => Err(anyhow!("invalid restart policy label")),
    }
}

/// Policy governing container restarts on exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicyLabel {
    No,
    Always,
    UnlessStopped,
    OnFailure(Option<u32>),
}

impl TryFrom<&str> for RestartPolicyLabel {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> Result<Self> {
        match raw {
            "no" => Ok(RestartPolicyLabel::No),
            "always" => Ok(RestartPolicyLabel::Always),
            "unless-stopped" => Ok(RestartPolicyLabel::UnlessStopped),
            "on-failure" => Ok(RestartPolicyLabel::OnFailure(None)),
            _ if raw.starts_with("on-failure:") => parse_on_failure(raw),
            _ => Err(anyhow!("invalid restart policy label")),
        }
    }
}

/// Decide whether an exited container should be restarted under `policy`.
pub fn should_restart(policy: &RestartPolicyLabel, exit_code: i32, restarts: u32) -> bool {
    match policy {
        RestartPolicyLabel::No => false,
        RestartPolicyLabel::Always | RestartPolicyLabel::UnlessStopped => true,
        RestartPolicyLabel::OnFailure(limit) => {
            if exit_code == 0 {
                return false;
            }
            match limit {
                Some(max) => restarts < *max,
                None => true,
            }
        }
    }
}

/// Compute backoff delay for a container that has restarted `restarts` times.
pub fn restart_delay(restarts: u32) -> Duration {
    let exponent = restarts.min(MAX_EXPONENT);
    let power = 1u64 << exponent;
    let seconds = power.min(MAX_BACKOFF_SECS);
    Duration::from_secs(seconds)
}

#[cfg(test)]
#[path = "restart_policy_tests.rs"]
mod tests;

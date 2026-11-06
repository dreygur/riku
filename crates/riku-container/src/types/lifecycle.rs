//! Stop, restart and health settings of a container.

use std::time::Duration;

/// How long a container gets to exit after the stop signal before it is killed.
pub const DEFAULT_STOP_GRACE: Duration = Duration::from_secs(30);

/// Signal sent to stop a container, such as `SIGTERM`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopSignal(String);

impl StopSignal {
    /// The signal name in upper case.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for StopSignal {
    fn default() -> Self {
        StopSignal("SIGTERM".to_string())
    }
}

impl TryFrom<&str> for StopSignal {
    type Error = anyhow::Error;

    /// Upper-cases `raw` and accepts `SIG` followed by letters or digits.
    fn try_from(raw: &str) -> anyhow::Result<Self> {
        let upper = raw.to_ascii_uppercase();
        let suffix = upper.strip_prefix("SIG").unwrap_or("");
        let valid = !suffix.is_empty()
            && suffix
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !valid {
            return Err(anyhow::anyhow!(
                "invalid stop signal {raw:?}: expected a name like SIGTERM"
            ));
        }
        Ok(StopSignal(upper))
    }
}

/// The engine's own restart policy. Riku runs containers with `No` and lets
/// the supervisor restart them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartPolicy {
    #[default]
    No,
    Always,
    OnFailure {
        /// `None` retries without limit.
        max_retries: Option<u32>,
    },
    UnlessStopped,
}

/// What a health check runs inside the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthTest {
    /// Run the program directly with these arguments.
    Exec(Vec<String>),
    /// Run this line through the container's shell.
    Shell(String),
    /// Turn off a health check the image defines.
    Disabled,
}

/// Health check settings. A `None` field keeps the engine or image default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheck {
    pub test: HealthTest,
    pub interval: Option<Duration>,
    pub timeout: Option<Duration>,
    pub retries: Option<u32>,
    /// Time after start during which failures do not count.
    pub start_period: Option<Duration>,
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

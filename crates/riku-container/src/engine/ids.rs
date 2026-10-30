//! Container identifiers as the engine prints them.

const MIN_LEN: usize = 12;
const MAX_LEN: usize = 64;

/// Id of a container the engine created: 12 to 64 lowercase hex characters.
///
/// A value of this type cannot start with `-`, so it is safe to pass to the
/// engine as a positional argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContainerId(String);

impl ContainerId {
    /// The id to pass to the engine.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ContainerId {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> anyhow::Result<Self> {
        let hex = raw
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'));
        if !(MIN_LEN..=MAX_LEN).contains(&raw.len()) || !hex {
            return Err(anyhow::anyhow!(
                "engine returned {raw:?} as a container id, expected {MIN_LEN} to {MAX_LEN} lowercase hex characters"
            ));
        }
        Ok(ContainerId(raw.to_string()))
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;

//! On/off ENV flags, read the same way by every router and by deploy output.
//!
//! A flag is on only for an explicit yes (`1`, `true`, `yes`, `on`, any case).
//! Anything else, including a typo, is off, and [`https_only_warning`] lets
//! deploy name the unrecognized value instead of silently ignoring it.

use std::collections::HashMap;

const HTTPS_ONLY_KEY: &str = "RIKU_HTTPS_ONLY";
const ON_VALUES: [&str; 4] = ["1", "true", "yes", "on"];
const OFF_VALUES: [&str; 5] = ["", "0", "false", "no", "off"];

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Whether a flag value means on.
pub fn is_on(value: &str) -> bool {
    ON_VALUES.contains(&normalized(value).as_str())
}

/// Whether the app asks for HTTPS only (`RIKU_HTTPS_ONLY`).
pub fn https_only(env: &HashMap<String, String>) -> bool {
    env.get(HTTPS_ONLY_KEY).is_some_and(|v| is_on(v))
}

/// A warning when `RIKU_HTTPS_ONLY` holds a value that is neither on nor off.
#[allow(clippy::question_mark)]
pub fn https_only_warning(env: &HashMap<String, String>) -> Option<String> {
    let Some(value) = env.get(HTTPS_ONLY_KEY) else {
        return None;
    };
    let known = normalized(value);
    if ON_VALUES.contains(&known.as_str()) || OFF_VALUES.contains(&known.as_str()) {
        return None;
    }
    Some(format!(
        "Warning: {HTTPS_ONLY_KEY}={value} is not a recognized value and is treated as off. Use true or false."
    ))
}

#[cfg(test)]
#[path = "flag_tests.rs"]
mod tests;

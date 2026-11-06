//! Health checks.

use super::duration::parse_duration;
use super::scalar::{reject_unknown, string_list};
use super::{Mapping, Value};
use crate::types::{HealthCheck, HealthTest};
use anyhow::{bail, Result};
use std::time::Duration;

const HEALTH_KEYS: [&str; 6] = [
    "test",
    "interval",
    "timeout",
    "retries",
    "start_period",
    "disable",
];

/// Parse a `healthcheck` mapping. `disable: true` or `test: [NONE]` give
/// [`HealthTest::Disabled`]; otherwise `test` is required.
pub(super) fn parse_healthcheck(value: &Value) -> Result<HealthCheck> {
    let Value::Object(fields) = value else {
        bail!("healthcheck must be a mapping");
    };
    if let Err(e) = reject_unknown(fields, &HEALTH_KEYS, "healthcheck") {
        return Err(e);
    }
    let disabled = match fields.get("disable") {
        None => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => bail!("healthcheck disable must be true or false"),
    };
    let test = if disabled {
        HealthTest::Disabled
    } else {
        match fields.get("test") {
            Some(test) => match parse_test(test) {
                Ok(test) => test,
                Err(e) => return Err(e),
            },
            None => bail!("healthcheck needs a test"),
        }
    };
    let interval = match duration_field(fields, "interval") {
        Ok(interval) => interval,
        Err(e) => return Err(e),
    };
    let timeout = match duration_field(fields, "timeout") {
        Ok(timeout) => timeout,
        Err(e) => return Err(e),
    };
    let start_period = match duration_field(fields, "start_period") {
        Ok(start_period) => start_period,
        Err(e) => return Err(e),
    };
    let retries = match fields.get("retries") {
        None => None,
        Some(Value::Number(n)) => match n.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(retries) => Some(retries),
            None => bail!("healthcheck retries must be a non-negative whole number"),
        },
        Some(_) => bail!("healthcheck retries must be a number"),
    };
    Ok(HealthCheck {
        test,
        interval,
        timeout,
        retries,
        start_period,
    })
}

fn duration_field(fields: &Mapping, key: &str) -> Result<Option<Duration>> {
    match fields.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => match parse_duration(text) {
            Ok(duration) => Ok(Some(duration)),
            Err(e) => Err(anyhow::anyhow!("healthcheck {key}: {e}")),
        },
        Some(_) => bail!("healthcheck {key} must be a string like 30s"),
    }
}

fn parse_test(value: &Value) -> Result<HealthTest> {
    if let Value::String(line) = value {
        return Ok(HealthTest::Shell(line.clone()));
    }
    let words = match string_list(value, "healthcheck test") {
        Ok(words) => words,
        Err(e) => return Err(e),
    };
    match words.split_first() {
        Some((first, rest)) if first == "NONE" && rest.is_empty() => Ok(HealthTest::Disabled),
        Some((first, rest)) if first == "CMD" && !rest.is_empty() => {
            Ok(HealthTest::Exec(rest.to_vec()))
        }
        Some((first, [line])) if first == "CMD-SHELL" => Ok(HealthTest::Shell(line.clone())),
        _ => bail!(
            "healthcheck test {words:?} must be a string, [CMD, ...], [CMD-SHELL, line] or [NONE]"
        ),
    }
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;

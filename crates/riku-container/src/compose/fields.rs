//! Single value service fields: image, command, user, restart, stop settings
//! and networks. Each text value is checked before it can become an engine
//! argument: an image or user may not start with `-`, and nothing may carry a
//! line break or NUL.

use super::duration::parse_duration;
use super::scalar::{check_text, string_list};
use super::Value;
use crate::types::{RestartPolicy, StopSignal, DEFAULT_STOP_GRACE};
use anyhow::{bail, Result};
use std::time::Duration;

/// An image reference: starts with a letter or digit, then letters, digits and
/// `/ : @ . _ -`.
pub(super) fn parse_image(value: &Value) -> Result<String> {
    let Value::String(image) = value else {
        bail!("image must be a string; quote it");
    };
    let mut chars = image.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || "/:@._-".contains(c));
    if !valid {
        bail!("invalid image {image:?}");
    }
    Ok(image.clone())
}

/// `command` or `entrypoint`: a list, or a string split like a shell would.
pub(super) fn parse_command(value: &Value, key: &str) -> Result<Vec<String>> {
    let Value::String(line) = value else {
        return string_list(value, key);
    };
    let Some(words) = shlex::split(line) else {
        bail!("{key} {line:?} has unbalanced quotes");
    };
    for word in &words {
        if let Err(e) = check_text(word, key) {
            return Err(e);
        }
    }
    Ok(words)
}

/// `user`: a name or id, optionally `user:group`.
pub(super) fn parse_user(value: &Value) -> Result<String> {
    let Value::String(user) = value else {
        bail!("user must be a string; quote it");
    };
    let mut chars = user.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c));
    if !valid {
        bail!("invalid user {user:?}");
    }
    Ok(user.clone())
}

/// `working_dir`: an absolute path in the container.
pub(super) fn parse_working_dir(value: &Value) -> Result<String> {
    match value {
        Value::String(dir) if dir.starts_with('/') && check_text(dir, "working_dir").is_ok() => {
            Ok(dir.clone())
        }
        other => bail!("working_dir {other} must be an absolute path"),
    }
}

/// `restart`: `no`, `always`, `on-failure[:n]` or `unless-stopped`.
pub(super) fn parse_restart(value: &Value) -> Result<RestartPolicy> {
    let policy = match value {
        Value::String(policy) => policy.as_str(),
        Value::Bool(false) => "no",
        other => bail!("restart {other} is not supported"),
    };
    match policy {
        "no" => Ok(RestartPolicy::No),
        "always" => Ok(RestartPolicy::Always),
        "unless-stopped" => Ok(RestartPolicy::UnlessStopped),
        "on-failure" => Ok(RestartPolicy::OnFailure { max_retries: None }),
        other => match other.strip_prefix("on-failure:").map(str::parse::<u32>) {
            Some(Ok(max)) => Ok(RestartPolicy::OnFailure {
                max_retries: Some(max),
            }),
            _ => bail!("restart {other:?} is not supported"),
        },
    }
}

/// `stop_grace_period`, or the default when the key is absent.
pub(super) fn parse_stop_grace(value: Option<&Value>) -> Result<Duration> {
    match value {
        None => Ok(DEFAULT_STOP_GRACE),
        Some(Value::String(text)) => parse_duration(text),
        Some(_) => bail!("stop_grace_period must be a string like 30s"),
    }
}

/// `stop_signal`.
pub(super) fn parse_stop_signal(value: &Value) -> Result<StopSignal> {
    match value {
        Value::String(name) => StopSignal::try_from(name.as_str()),
        other => bail!("invalid stop signal {other}"),
    }
}

/// `networks`: only `default` may be named, with no settings.
pub(super) fn check_networks(value: &Value) -> Result<()> {
    let names: Vec<(&str, bool)> = match value {
        Value::Array(items) => {
            let mut names = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(name) => names.push((name.as_str(), true)),
                    _ => bail!("networks entries must be names"),
                }
            }
            names
        }
        Value::Object(entries) => entries
            .iter()
            .map(|(name, settings)| {
                let empty = match settings {
                    Value::Null => true,
                    Value::Object(settings) => settings.is_empty(),
                    _ => false,
                };
                (name.as_str(), empty)
            })
            .collect(),
        _ => bail!("networks must be a list or a mapping"),
    };
    for (name, plain) in names {
        if name != "default" || !plain {
            bail!("network {name:?}: riku gives each app one network; only 'default' is allowed, without settings");
        }
    }
    Ok(())
}

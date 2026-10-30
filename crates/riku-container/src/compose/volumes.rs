//! Volume and bind mounts.
//!
//! Bind sources are resolved lexically and must stay inside the app directory,
//! absolute paths included, so a compose file cannot mount `/etc` or the
//! engine socket. Named volumes must be declared at the top level and use the
//! engine's name character set. Target paths are absolute and free of the
//! characters the engine's mount syntax splits on.

use super::paths::resolve_inside;
use super::scalar::reject_unknown;
use super::{Mapping, ServiceContext, Value};
use super::{ServiceMount, ServiceMountSource, VolumeKey};
use anyhow::{bail, Result};
use std::path::PathBuf;

/// Parse the `volumes` list.
pub(super) fn parse_volumes(ctx: &ServiceContext, value: &Value) -> Result<Vec<ServiceMount>> {
    let Value::Array(entries) = value else {
        bail!("volumes must be a list");
    };
    let mut mounts = Vec::with_capacity(entries.len());
    for entry in entries {
        let parsed = match entry {
            Value::String(text) => parse_short(ctx, text),
            Value::Object(fields) => parse_long(ctx, fields),
            _ => bail!("volumes entries must be strings or mappings"),
        };
        match parsed {
            Ok(mount) => mounts.push(mount),
            Err(e) => return Err(e),
        }
    }
    Ok(mounts)
}

fn is_volume_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn named_volume(ctx: &ServiceContext, name: &str) -> Result<ServiceMountSource> {
    if !is_volume_name(name) {
        bail!("volume source {name:?} is not a volume name or a relative or absolute path");
    }
    if !ctx.declared_volumes.contains(name) {
        bail!("volume {name:?} is not declared under the top level volumes key");
    }
    Ok(ServiceMountSource::Volume(VolumeKey::declared(name)))
}

fn bind_or_named(ctx: &ServiceContext, source: &str) -> Result<ServiceMountSource> {
    if source.starts_with(['/', '.']) {
        return match resolve_inside(ctx.app_path, ctx.compose_dir, source, "bind source") {
            Ok(path) => Ok(ServiceMountSource::Bind(path)),
            Err(e) => Err(e),
        };
    }
    named_volume(ctx, source)
}

fn container_path(raw: &str) -> Result<PathBuf> {
    if !raw.starts_with('/') || raw.contains([':', ',', '\n', '\r', '\0']) {
        bail!("volume target {raw:?} must be an absolute path without ':' or ','");
    }
    Ok(PathBuf::from(raw))
}

fn parse_short(ctx: &ServiceContext, text: &str) -> Result<ServiceMount> {
    let parts: Vec<&str> = text.split(':').collect();
    let (source, target, read_only) = match parts.as_slice() {
        [target] => (None, *target, false),
        [source, target] => (Some(*source), *target, false),
        [source, target, "ro"] => (Some(*source), *target, true),
        [source, target, "rw"] => (Some(*source), *target, false),
        _ => bail!("volume {text:?}: expected SOURCE:TARGET, SOURCE:TARGET:ro or TARGET"),
    };
    let source = match source {
        Some(source) => match bind_or_named(ctx, source) {
            Ok(source) => source,
            Err(e) => return Err(e),
        },
        None => ServiceMountSource::Anonymous,
    };
    let target = match container_path(target) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    Ok(ServiceMount {
        source,
        target,
        read_only,
    })
}

fn text_field<'a>(fields: &'a Mapping, key: &str) -> Result<Option<&'a str>> {
    match fields.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => bail!("volume {key} must be a string"),
    }
}

fn parse_long(ctx: &ServiceContext, fields: &Mapping) -> Result<ServiceMount> {
    if let Err(e) = reject_unknown(fields, &["type", "source", "target", "read_only"], "volume") {
        return Err(e);
    }
    let source = match (text_field(fields, "type"), text_field(fields, "source")) {
        (Ok(Some("bind")), Ok(Some(source))) => {
            resolve_inside(ctx.app_path, ctx.compose_dir, source, "bind source")
                .map(ServiceMountSource::Bind)
        }
        (Ok(Some("bind")), Ok(None)) => bail!("a bind volume needs a source"),
        (Ok(Some("volume")), Ok(Some(source))) => named_volume(ctx, source),
        (Ok(Some("volume")), Ok(None)) => Ok(ServiceMountSource::Anonymous),
        (Ok(other), Ok(_)) => bail!("volume type {other:?} is not supported; use volume or bind"),
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    let source = match source {
        Ok(source) => source,
        Err(e) => return Err(e),
    };
    let target = match text_field(fields, "target") {
        Ok(Some(raw)) => match container_path(raw) {
            Ok(target) => target,
            Err(e) => return Err(e),
        },
        Ok(None) => bail!("a volume needs a target"),
        Err(e) => return Err(e),
    };
    let read_only = match fields.get("read_only") {
        None => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => bail!("volume read_only must be true or false"),
    };
    Ok(ServiceMount {
        source,
        target,
        read_only,
    })
}

#[cfg(test)]
#[path = "volumes_tests.rs"]
mod tests;

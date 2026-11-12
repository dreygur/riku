//! The steps [`super::parse_service`] runs, one per group of service keys.
//! Each reads its keys from the raw mapping and fills its part of the
//! [`ServiceDef`].

use super::build::parse_build;
use super::definition::{ServiceContext, ServiceDef};
use super::depends::parse_depends_on;
use super::fields::{
    check_networks, parse_command, parse_image, parse_restart, parse_stop_grace, parse_stop_signal,
    parse_user, parse_working_dir,
};
use super::health::parse_healthcheck;
use super::ports::{host_ip_for, parse_ports};
use super::scalar::string_map;
use super::service_env::parse_environment;
use super::volumes::parse_volumes;
use super::Mapping;
use crate::types::labels;
use anyhow::{bail, Result};

pub(super) type Step = fn(&ServiceContext, &Mapping, &mut ServiceDef) -> Result<()>;

/// Labels run before ports, which read `riku.public` from them.
pub(super) const STEPS: [Step; 7] = [
    apply_labels,
    apply_image_and_build,
    apply_process,
    apply_lifecycle,
    apply_environment,
    apply_ports,
    apply_volumes,
];

fn apply_labels(_: &ServiceContext, fields: &Mapping, service: &mut ServiceDef) -> Result<()> {
    if let Some(labels) = fields.get("labels") {
        service.labels = match string_map(labels, "label") {
            Ok(labels) => labels,
            Err(e) => return Err(e),
        };
    }
    match [
        labels::APP,
        labels::SERVICE,
        labels::COPY,
        labels::PORT,
        labels::RESTART,
        labels::SPEC,
    ]
    .into_iter()
    .find(|reserved| service.labels.contains_key(*reserved))
    {
        Some(reserved) => bail!("label {reserved} is set by riku and cannot be overridden"),
        None => Ok(()),
    }
}

fn apply_image_and_build(
    ctx: &ServiceContext,
    fields: &Mapping,
    service: &mut ServiceDef,
) -> Result<()> {
    if let Some(image) = fields.get("image") {
        service.image = match parse_image(image) {
            Ok(image) => Some(image),
            Err(e) => return Err(e),
        };
    }
    if let Some(build) = fields.get("build") {
        service.build = match parse_build(ctx, build) {
            Ok(build) => Some(build),
            Err(e) => return Err(e),
        };
    }
    if service.image.is_none() && service.build.is_none() {
        bail!("needs an image or build");
    }
    Ok(())
}

fn apply_process(_: &ServiceContext, fields: &Mapping, service: &mut ServiceDef) -> Result<()> {
    if let Some(command) = fields.get("command") {
        service.command = match parse_command(command, "command") {
            Ok(words) => Some(words),
            Err(e) => return Err(e),
        };
    }
    if let Some(entrypoint) = fields.get("entrypoint") {
        service.entrypoint = match parse_command(entrypoint, "entrypoint") {
            Ok(words) => Some(words),
            Err(e) => return Err(e),
        };
    }
    if let Some(dir) = fields.get("working_dir") {
        service.working_dir = match parse_working_dir(dir) {
            Ok(dir) => Some(dir),
            Err(e) => return Err(e),
        };
    }
    if let Some(user) = fields.get("user") {
        service.user = match parse_user(user) {
            Ok(user) => Some(user),
            Err(e) => return Err(e),
        };
    }
    Ok(())
}

fn apply_lifecycle(_: &ServiceContext, fields: &Mapping, service: &mut ServiceDef) -> Result<()> {
    if let Some(restart) = fields.get("restart") {
        service.restart = match parse_restart(restart) {
            Ok(policy) => policy,
            Err(e) => return Err(e),
        };
    }
    service.stop_grace = match parse_stop_grace(fields.get("stop_grace_period")) {
        Ok(grace) => grace,
        Err(e) => return Err(e),
    };
    if let Some(signal) = fields.get("stop_signal") {
        service.stop_signal = match parse_stop_signal(signal) {
            Ok(signal) => signal,
            Err(e) => return Err(e),
        };
    }
    if let Some(check) = fields.get("healthcheck") {
        service.healthcheck = match parse_healthcheck(check) {
            Ok(check) => Some(check),
            Err(e) => return Err(e),
        };
    }
    if let Some(depends_on) = fields.get("depends_on") {
        service.depends_on = match parse_depends_on(depends_on) {
            Ok(dependencies) => dependencies,
            Err(e) => return Err(e),
        };
    }
    match fields.get("networks") {
        Some(networks) => check_networks(networks),
        None => Ok(()),
    }
}

fn apply_environment(
    ctx: &ServiceContext,
    fields: &Mapping,
    service: &mut ServiceDef,
) -> Result<()> {
    service.environment = match parse_environment(ctx, fields) {
        Ok(vars) => vars,
        Err(e) => return Err(e),
    };
    Ok(())
}

fn apply_ports(_: &ServiceContext, fields: &Mapping, service: &mut ServiceDef) -> Result<()> {
    let host_ip = match host_ip_for(&service.labels) {
        Ok(host_ip) => host_ip,
        Err(e) => return Err(e),
    };
    if let Some(ports) = fields.get("ports") {
        service.ports = match parse_ports(ports, host_ip) {
            Ok(ports) => ports,
            Err(e) => return Err(e),
        };
    }
    Ok(())
}

fn apply_volumes(ctx: &ServiceContext, fields: &Mapping, service: &mut ServiceDef) -> Result<()> {
    if let Some(volumes) = fields.get("volumes") {
        service.mounts = match parse_volumes(ctx, volumes) {
            Ok(mounts) => mounts,
            Err(e) => return Err(e),
        };
    }
    Ok(())
}

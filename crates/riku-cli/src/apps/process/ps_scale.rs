use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::apps::control_target::{control_target, ControlTarget};
use crate::config::RikuPaths;
use crate::deploy::containers::{scale_service, AppName, ServiceName};
use crate::util::{display, parse_settings, require_app};

#[allow(clippy::question_mark)]
fn service_count(setting: &str) -> Result<(ServiceName, u32)> {
    let Some((name, count)) = setting.split_once('=') else {
        return Err(anyhow!(
            "malformed setting '{setting}', use <service>=<copies>"
        ));
    };
    let service = match ServiceName::try_from(name.trim()) {
        Ok(service) => service,
        Err(e) => return Err(e.context(format!("in scale setting '{setting}'"))),
    };
    match count.trim().parse::<u32>() {
        Ok(0) => Err(anyhow!(
            "cannot apply '{setting}': copies must be at least 1; `riku stop <app> {service}` stops a service"
        )),
        Ok(count) => Ok((service, count)),
        Err(_) => Err(anyhow!(
            "malformed setting '{setting}', copies must be a whole number"
        )),
    }
}

/// Scale each named service of a container app in turn, without a rebuild.
/// Every setting is parsed before the first service changes.
#[allow(clippy::question_mark)]
fn scale_containers(paths: &RikuPaths, app: &AppName, settings: &[String]) -> Result<()> {
    let wanted: Vec<(ServiceName, u32)> = match settings.iter().map(|s| service_count(s)).collect()
    {
        Ok(wanted) => wanted,
        Err(e) => return Err(e),
    };
    for (service, copies) in &wanted {
        display::step_done(&format!("Scaling '{service}' to {copies}"), "");
        if let Err(e) = scale_service(app, paths, service, *copies) {
            return Err(e);
        }
    }
    Ok(())
}

/// Procfile apps: compute deltas against `SCALING` and deploy with them.
/// Every setting is checked before the deploy starts; a bad one is an error.
#[allow(clippy::question_mark)]
fn scale_procfile(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    let mut env = HashMap::new();
    let worker_count = match parse_settings(&config_file, &mut env) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };

    let mut deltas: HashMap<String, i64> = HashMap::new();
    let mut added = Vec::new();
    for s in settings {
        let Some((k, v_str)) = s.split_once('=') else {
            return Err(anyhow!("malformed setting '{s}', use <kind>=<copies>"));
        };
        let k = k.trim().to_string();
        let c = match v_str.trim().parse::<i64>() {
            Ok(c) if c < 0 => {
                return Err(anyhow!(
                    "cannot apply '{s}': cannot scale type '{k}' below 0"
                ))
            }
            Ok(c) => c,
            Err(_) => {
                return Err(anyhow!(
                    "malformed setting '{s}', copies must be a whole number"
                ))
            }
        };
        match worker_count.get(&k).map(|current| current.parse::<i64>()) {
            Some(Ok(current_val)) => {
                deltas.insert(k, c - current_val);
            }
            Some(Err(_)) => {
                return Err(anyhow!(
                    "cannot apply '{s}': SCALING holds no whole number for '{k}'"
                ))
            }
            None => {
                added.push(format!("Adding new worker type '{k}' with count {c}"));
                deltas.insert(k, c);
            }
        }
    }

    for line in &added {
        display::step_done(line, "");
    }
    crate::deploy::do_deploy(app, paths, &deltas, None)
}

/// Scale an app. Container apps grow or shrink each named service without a
/// rebuild; Procfile apps redeploy with the new worker counts.
#[allow(clippy::question_mark)]
pub fn cmd_ps_scale(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = match require_app(app, &paths.app_root) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    match control_target(paths, &app, None) {
        Ok(ControlTarget::Containers { app, .. }) => scale_containers(paths, &app, settings),
        Ok(ControlTarget::Procfile { .. }) => scale_procfile(paths, &app, settings),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "ps_scale_tests.rs"]
mod tests;

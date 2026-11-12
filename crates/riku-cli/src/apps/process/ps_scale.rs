use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::apps::control_target::{control_target, ControlTarget};
use crate::config::RikuPaths;
use crate::deploy::containers::{scale_service, AppName, ServiceName};
use crate::util::{display, exit_if_invalid, parse_settings};

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
#[allow(clippy::question_mark)]
fn scale_procfile(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let config_file = paths.env_root.join(app).join("SCALING");
    let mut env = HashMap::new();
    let worker_count = match parse_settings(&config_file, &mut env) {
        Ok(counts) => counts,
        Err(e) => return Err(e),
    };

    let mut deltas: HashMap<String, i64> = HashMap::new();
    for s in settings {
        if let Some(eq_pos) = s.find('=') {
            let k = s[..eq_pos].trim().to_string();
            let v_str = s[eq_pos + 1..].trim().to_string();
            match v_str.parse::<i64>() {
                Ok(c) => {
                    if c < 0 {
                        display::error(&format!("cannot scale type '{}' below 0", k));
                        return Ok(());
                    }
                    if let Some(current) = worker_count.get(&k) {
                        match current.parse::<i64>() {
                            Ok(current_val) => {
                                deltas.insert(k, c - current_val);
                            }
                            Err(_) => {
                                display::error(&format!("malformed setting '{}'", s));
                                return Ok(());
                            }
                        }
                    } else {
                        display::step_done(
                            &format!("Adding new worker type '{}' with count {}", k, c),
                            "",
                        );
                        deltas.insert(k, c);
                    }
                }
                Err(_) => {
                    display::error(&format!("malformed setting '{}'", s));
                    return Ok(());
                }
            }
        } else {
            display::error(&format!("malformed setting '{}'", s));
            return Ok(());
        }
    }

    crate::deploy::do_deploy(app, paths, &deltas, None)
}

/// Scale an app. Container apps grow or shrink each named service without a
/// rebuild; Procfile apps redeploy with the new worker counts.
#[allow(clippy::question_mark)]
pub fn cmd_ps_scale(paths: &RikuPaths, app: &str, settings: &[String]) -> Result<()> {
    let app = match exit_if_invalid(app, &paths.app_root) {
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

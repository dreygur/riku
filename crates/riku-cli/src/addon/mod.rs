//! `riku addon` provider layer: wires the addon CLI to [`AddonService`] and
//! handles user-facing output. No business logic lives here.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::display;
use riku_plugins::AddonService;

/// `riku addon list`
pub fn cmd_addon_list(paths: &RikuPaths) -> Result<()> {
    let instances = AddonService::new(paths).list();
    if instances.is_empty() {
        display::detail("No addon instances. Create one with: riku addon create <plugin> <name>");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = instances
        .iter()
        .map(|i| {
            let apps: Vec<&str> = i.bindings.keys().map(String::as_str).collect();
            let bound = if apps.is_empty() {
                "-".to_string()
            } else {
                apps.join(", ")
            };
            vec![i.instance.clone(), i.plugin.clone(), bound]
        })
        .collect();
    display::print_table(&["INSTANCE", "ADDON", "BOUND APPS"], &rows, 2);
    Ok(())
}

/// `riku addon create <plugin> <name>`
#[allow(clippy::question_mark)]
pub fn cmd_addon_create(paths: &RikuPaths, plugin: &str, name: &str) -> Result<()> {
    let app = match crate::util::validate_app_name(name) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    display::intro(&format!("riku addon create  {plugin} {app}"));
    if let Err(e) = AddonService::new(paths).provision(plugin, &app) {
        return Err(e);
    }
    display::step_done("Provisioned", &app);
    display::outro(&format!("{app} is ready"));
    display::summary(&[("Bind", &format!("riku addon bind {app} <app>"))]);
    Ok(())
}

/// `riku addon bind <instance> <app>`
#[allow(clippy::question_mark)]
pub fn cmd_addon_bind(paths: &RikuPaths, instance: &str, app: &str) -> Result<()> {
    display::intro(&format!("riku addon bind  {instance} {app}"));
    let keys = match AddonService::new(paths).bind(instance, app) {
        Ok(keys) => keys,
        Err(e) => return Err(e),
    };
    display::step_done("Bound", &format!("{instance} to {app}"));
    if !keys.is_empty() {
        display::step_done("Env", &keys.join(", "));
    }
    display::outro(&format!("{app} can use {instance}"));
    Ok(())
}

/// `riku addon unbind <instance> <app>`
#[allow(clippy::question_mark)]
pub fn cmd_addon_unbind(paths: &RikuPaths, instance: &str, app: &str) -> Result<()> {
    if let Err(e) = AddonService::new(paths).unbind(instance, app) {
        return Err(e);
    }
    display::step_done(&format!("Unbound '{instance}' from '{app}'."), "");
    Ok(())
}

/// `riku addon destroy <instance>`
#[allow(clippy::question_mark)]
pub fn cmd_addon_destroy(paths: &RikuPaths, instance: &str) -> Result<()> {
    if let Err(e) = AddonService::new(paths).deprovision(instance) {
        return Err(e);
    }
    display::step_done(&format!("Destroyed instance '{instance}'."), "");
    Ok(())
}

/// `riku addon backup <instance>`
#[allow(clippy::question_mark)]
pub fn cmd_addon_backup(paths: &RikuPaths, instance: &str) -> Result<()> {
    let artifact = match AddonService::new(paths).backup(instance) {
        Ok(artifact) => artifact,
        Err(e) => return Err(e),
    };
    match artifact {
        Some(artifact) => display::step_done(&format!("Backed up '{instance}' to {artifact}"), ""),
        None => display::warn(&format!(
            "Addon for '{instance}' produced no backup artifact"
        )),
    }
    Ok(())
}

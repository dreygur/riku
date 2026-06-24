//! `riku addon` provider layer — wires the addon CLI to [`AddonService`] and
//! handles user-facing output. No business logic lives here.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::AddonService;
use crate::util::display;

/// `riku addon list`
pub fn cmd_addon_list(paths: &RikuPaths) -> Result<()> {
    let instances = AddonService::new(paths).list();
    if instances.is_empty() {
        display::note("No addon instances. Create one with: riku addon create <plugin> <name>");
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
pub fn cmd_addon_create(paths: &RikuPaths, plugin: &str, name: &str) -> Result<()> {
    let app = crate::util::validate_app_name(name)?;
    display::info(&format!("Provisioning {plugin} instance '{app}'..."));
    AddonService::new(paths).provision(plugin, &app)?;
    display::success(&format!(
        "Provisioned '{app}'. Bind it: riku addon bind {app} <app>"
    ));
    Ok(())
}

/// `riku addon bind <instance> <app>`
pub fn cmd_addon_bind(paths: &RikuPaths, instance: &str, app: &str) -> Result<()> {
    let keys = AddonService::new(paths).bind(instance, app)?;
    display::success(&format!("Bound '{instance}' to '{app}'."));
    if !keys.is_empty() {
        display::note(&format!("Injected into {app} env: {}", keys.join(", ")));
    }
    Ok(())
}

/// `riku addon unbind <instance> <app>`
pub fn cmd_addon_unbind(paths: &RikuPaths, instance: &str, app: &str) -> Result<()> {
    AddonService::new(paths).unbind(instance, app)?;
    display::success(&format!("Unbound '{instance}' from '{app}'."));
    Ok(())
}

/// `riku addon destroy <instance>`
pub fn cmd_addon_destroy(paths: &RikuPaths, instance: &str) -> Result<()> {
    AddonService::new(paths).deprovision(instance)?;
    display::success(&format!("Destroyed instance '{instance}'."));
    Ok(())
}

/// `riku addon backup <instance>`
pub fn cmd_addon_backup(paths: &RikuPaths, instance: &str) -> Result<()> {
    match AddonService::new(paths).backup(instance)? {
        Some(artifact) => display::success(&format!("Backed up '{instance}' to {artifact}")),
        None => display::warn(&format!(
            "Addon for '{instance}' produced no backup artifact"
        )),
    }
    Ok(())
}

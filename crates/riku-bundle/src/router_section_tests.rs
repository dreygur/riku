use super::*;
use crate::manifest::PluginManifest;
use crate::RIKU_PLUGIN_API;

fn router_manifest(extra: &str) -> String {
    format!(
        r#"
        name = "edge-router"
        version = "0.1.0"
        type = "router"
        api = {RIKU_PLUGIN_API}
        entry = "bin/router"
        {extra}
        "#
    )
}

#[test]
fn router_section_parses() {
    let manifest = PluginManifest::from_toml_str(&router_manifest(
        "[router]\nprotocol = 2\ngraceful_reload = true",
    ))
    .unwrap();

    assert_eq!(
        manifest.router,
        Some(RouterSection {
            protocol: 2,
            graceful_reload: true
        })
    );
}

#[test]
fn router_table_is_optional_and_reload_defaults_to_not_graceful() {
    let without = PluginManifest::from_toml_str(&router_manifest("")).unwrap();
    assert_eq!(without.router, None);

    let bare = PluginManifest::from_toml_str(&router_manifest("[router]\nprotocol = 2")).unwrap();
    assert_eq!(
        bare.router.map(|section| section.graceful_reload),
        Some(false)
    );
}

//! The `nginx.include_content` filter chain reaching the generated config,
//! through a real filter plugin dispatched by the actual `FilterBus`.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;

use crate::config::RikuPaths;
use crate::test_support::{generate_live, paths_with_nginx, ACCEPT};

/// Installs a filter plugin into `paths.plugin_root` that appends a marker
/// line to whatever seed content it receives.
fn install_augmenter_filter(paths: &RikuPaths) {
    let bundle = paths.plugin_root.join("augmenter");
    fs::create_dir_all(bundle.join("bin")).unwrap();
    fs::write(
        bundle.join("bin/on-filter"),
        "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%s# augmented-by-plugin\\\\n\"}' \"$data\"\n",
    )
    .unwrap();
    fs::set_permissions(
        bundle.join("bin/on-filter"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::write(
        bundle.join("riku-plugin.toml"),
        format!(
            "name=\"augmenter\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-filter\"\n[filters]\nsubscribe=[\"nginx.include_content\"]\n",
            riku_plugins::RIKU_PLUGIN_API
        ),
    )
    .unwrap();
}

fn rendered_with_filter(env: &[(&str, &str)]) -> String {
    let temp_dir = TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), ACCEPT);
    install_augmenter_filter(&paths);
    generate_live(temp_dir.path(), &paths, env)
}

/// The default template pulls in `nginx_common.conf.tera` through an
/// include, so this checks the shared partial carries the filtered content.
#[test]
fn test_default_template_includes_filter_augmented_content() {
    let config = rendered_with_filter(&[("RIKU_DOMAINS", "example.com")]);
    assert!(config.contains("augmented-by-plugin"), "got: {config}");
}

#[test]
fn test_static_template_includes_filter_augmented_content() {
    let config = rendered_with_filter(&[
        ("RIKU_DOMAINS", "example.com"),
        ("RIKU_NGINX_STATIC", "true"),
    ]);
    assert!(config.contains("augmented-by-plugin"), "got: {config}");
}

use std::fs;

use super::*;

#[test]
fn lists_only_the_downloaded_plugins_that_are_installed() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::write(tmp.path().join("java"), "binary").unwrap();
    fs::write(tmp.path().join("python"), "script").unwrap();

    let installed = installed_downloads(tmp.path());

    assert_eq!(installed, ["java"]);
    assert_eq!(
        downloads_notice(&installed),
        Some((
            "Not refreshed, downloaded plugins: java".to_string(),
            "To update them to this riku's release: riku install-plugins --plugins java"
                .to_string()
        ))
    );
}

#[test]
fn says_nothing_when_no_downloaded_plugin_is_installed() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::write(tmp.path().join("node"), "script").unwrap();

    assert_eq!(downloads_notice(&installed_downloads(tmp.path())), None);
}

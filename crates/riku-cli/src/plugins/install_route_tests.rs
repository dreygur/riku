use super::*;

#[test]
fn bare_runtime_name_is_a_runtime_even_with_a_matching_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("node")).unwrap();

    assert_eq!(
        install_target("node").unwrap(),
        InstallTarget::Runtime("node")
    );
    assert_eq!(
        install_target("java").unwrap(),
        InstallTarget::Runtime("java")
    );
    assert_eq!(
        install_target("riku-notify").unwrap(),
        InstallTarget::Runtime("riku-notify")
    );
}

#[test]
fn paths_and_git_sources_are_bundles() {
    for source in [
        "./mybundle",
        "../mybundle",
        "/srv/mybundle",
        "plugins/mybundle",
        "github:owner/repo",
        "git@github.com:owner/repo",
        "https://example.com/repo.git",
        "repo.git",
    ] {
        assert_eq!(
            install_target(source).unwrap(),
            InstallTarget::Bundle(source.to_string()),
            "{source}"
        );
    }
}

#[test]
fn unknown_bare_name_is_refused_with_the_known_names() {
    let message = install_target("mybundle").unwrap_err().to_string();

    assert!(
        message.contains("mybundle") && message.contains("node") && message.contains("./mybundle"),
        "{message}"
    );
}

use super::*;

fn file_names(configs: &[PathBuf]) -> Vec<String> {
    configs
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn ownerless_files_are_claimed_only_by_their_app_at_name() {
    let tmp = tempfile::tempdir().unwrap();
    for name in [
        "shop@web.1.ini",
        "shop-api@web.1.ini",
        "shop@web.2.toml",
        "shop-web-4.ini",
        "shop@worker.ini",
    ] {
        fs::write(tmp.path().join(name), "[uwsgi]\nmaster = true\n").unwrap();
    }
    std::os::unix::fs::symlink(tmp.path().join("gone"), tmp.path().join("shop@web.3.toml"))
        .unwrap();

    assert_eq!(
        file_names(&app_worker_configs(tmp.path(), "shop")),
        ["shop@web.1.ini", "shop@web.2.toml", "shop@web.3.toml"],
        "shop must not claim shop-api's file, a legacy name or a name without an ordinal"
    );
    assert_eq!(
        file_names(&app_worker_configs(tmp.path(), "shop-api")),
        ["shop-api@web.1.ini"]
    );
}

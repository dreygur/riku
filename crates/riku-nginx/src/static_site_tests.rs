use std::os::unix::fs::{symlink, PermissionsExt};

use super::*;

fn static_env(paths_value: &str) -> HashMap<String, String> {
    HashMap::from([
        ("RIKU_NGINX_STATIC".to_string(), "true".to_string()),
        (
            "RIKU_NGINX_STATIC_PATHS".to_string(),
            paths_value.to_string(),
        ),
    ])
}

fn app_entries(paths: &RikuPaths) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&paths.static_root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn setup() -> (tempfile::TempDir, RikuPaths, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.static_root).unwrap();
    let app_path = tmp.path().join("checkout");
    fs::create_dir_all(app_path.join("public")).unwrap();
    fs::create_dir_all(app_path.join("assets/css")).unwrap();
    fs::write(app_path.join("public/index.html"), "home").unwrap();
    fs::write(app_path.join("assets/css/site.css"), "body{}").unwrap();
    (tmp, paths, app_path)
}

#[test]
fn publishes_root_and_paths_as_group_only_copies_and_replaces_the_old_copy() {
    let (_tmp, paths, app_path) = setup();
    symlink("../assets/css/site.css", app_path.join("public/site.css")).unwrap();
    let env = static_env("/assets:assets");

    publish_static("myapp", &app_path, &env, &paths).unwrap();
    fs::write(app_path.join("public/index.html"), "home v2").unwrap();
    publish_static("myapp", &app_path, &env, &paths).unwrap();

    let index = document_root("myapp", &paths).join("index.html");
    assert_eq!(fs::read_to_string(&index).unwrap(), "home v2");
    assert_eq!(
        fs::read_to_string(document_root("myapp", &paths).join("site.css")).unwrap(),
        "body{}",
        "a symlink inside the checkout should be copied as its target"
    );
    let css = static_path_dir("myapp", &paths, 0).join("css/site.css");
    assert_eq!(fs::read_to_string(&css).unwrap(), "body{}");
    let mode = fs::metadata(&css).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640, "published file mode is {mode:o}");
    let entries = app_entries(&paths);
    assert_eq!(
        entries.len(),
        2,
        "expected the link and one copy: {entries:?}"
    );
}

#[test]
fn refuses_a_symlink_out_of_the_checkout_and_publishes_nothing() {
    let (tmp, paths, app_path) = setup();
    let secret = tmp.path().join("secret");
    fs::write(&secret, "ENV").unwrap();
    symlink(&secret, app_path.join("public/leak")).unwrap();

    let err = publish_static("myapp", &app_path, &static_env(""), &paths)
        .unwrap_err()
        .to_string();

    assert!(err.contains("outside the app checkout"), "{err}");
    assert_eq!(
        app_entries(&paths),
        Vec::<String>::new(),
        "stage left behind"
    );
}

#[test]
fn remove_static_drops_the_link_and_every_copy_but_not_other_apps() {
    let (_tmp, paths, app_path) = setup();
    publish_static("myapp", &app_path, &static_env(""), &paths).unwrap();
    publish_static("myapp-two", &app_path, &static_env(""), &paths).unwrap();

    remove_static("myapp", &paths).unwrap();

    let left = app_entries(&paths);
    assert!(
        left.iter().all(|name| name.starts_with("myapp-two")),
        "{left:?}"
    );
    assert_eq!(left.len(), 2, "myapp-two lost files: {left:?}");
}

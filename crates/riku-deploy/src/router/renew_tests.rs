use std::cell::RefCell;
use std::fs;

use super::*;

fn paths_with_app(app: &str) -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.nginx_root).unwrap();
    fs::create_dir_all(&paths.static_root).unwrap();
    fs::create_dir_all(&paths.socket_root).unwrap();
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(
        paths.app_settings_override(app),
        "[routing]\ndomains = [\"myapp.example\"]\n",
    )
    .unwrap();
    (tmp, paths)
}

#[test]
fn renew_sends_one_apply_for_the_app() {
    let (_tmp, paths) = paths_with_app("myapp");
    let seen = RefCell::new(Vec::new());

    renew_with(
        |request| {
            seen.borrow_mut().push(request.clone());
            Ok(Reply::Ok { reloaded: true })
        },
        "myapp",
        &paths,
    )
    .unwrap();

    let seen = seen.borrow();
    assert_eq!(seen.len(), 1, "expected one request, got {seen:?}");
    match &seen[0] {
        Request::Apply { app, route, .. } => {
            assert_eq!(app, "myapp");
            assert_eq!(route.domains, ["myapp.example"]);
        }
        other => panic!("expected an apply, got {other:?}"),
    }
}

#[test]
fn renew_refuses_a_bad_app_name_without_contacting_the_helper() {
    let (_tmp, paths) = paths_with_app("myapp");
    let calls = RefCell::new(0);

    let err = renew_with(
        |_| {
            *calls.borrow_mut() += 1;
            Ok(Reply::Ok { reloaded: true })
        },
        "../myapp",
        &paths,
    )
    .unwrap_err();

    assert!(err.to_string().contains("cannot route"), "{err}");
    assert_eq!(*calls.borrow(), 0, "the helper was contacted");
}

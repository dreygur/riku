use std::cell::RefCell;

use riku_route::{Reply, Request};

use super::*;
use crate::router::routing_state::{with_routing_lock, Entry, RoutingMode};

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[routing]\nshared_port = 9090\n").unwrap();
    (tmp, paths)
}

fn set_mode(paths: &RikuPaths, app: &str, mode: RoutingMode) {
    with_routing_lock(paths, |state| {
        state.set_entry(
            app,
            Entry {
                mode,
                port: None,
                picked: false,
                tls: None,
            },
        );
        Ok(())
    })
    .unwrap();
}

fn sent(paths: &RikuPaths, current: (&str, Vec<Backend>), leaving: bool) -> Vec<Request> {
    let seen = RefCell::new(Vec::new());
    let send = |request: &Request| {
        seen.borrow_mut().push(request.clone());
        Ok(Reply::Ok { reloaded: true })
    };
    let lookup = |_: &str| Ok(vec![Backend::loopback(6000)]);
    publish_paths_with(send, lookup, paths, &Router::Nginx, current, leaving).unwrap();
    seen.into_inner()
}

#[test]
fn every_path_app_is_published_on_the_shared_port() {
    let (_tmp, paths) = paths();
    set_mode(&paths, "shop", RoutingMode::Path);
    set_mode(&paths, "blog", RoutingMode::Path);
    set_mode(&paths, "site", RoutingMode::Domain);

    let requests = sent(&paths, ("shop", vec![Backend::loopback(5001)]), false);

    match &requests[..] {
        [Request::ApplyPaths { listen, apps, .. }] => {
            assert_eq!(listen.port, 9090);
            let names: Vec<&str> = apps.iter().map(|app| app.app.as_str()).collect();
            assert_eq!(names, ["blog", "shop"]);
        }
        other => panic!("expected one apply_paths, got {other:?}"),
    }
}

#[test]
fn no_path_apps_sends_no_request() {
    let (_tmp, paths) = paths();
    set_mode(&paths, "site", RoutingMode::Domain);

    assert!(sent(&paths, ("site", Vec::new()), false).is_empty());
}

#[test]
fn switching_out_of_path_mode_drops_the_path_entry() {
    let (_tmp, paths) = paths();
    set_mode(&paths, "shop", RoutingMode::Domain);

    let requests = sent(&paths, ("shop", Vec::new()), true);

    match &requests[..] {
        [Request::ApplyPaths { apps, .. }] => assert!(apps.is_empty(), "{apps:?}"),
        other => panic!("expected an empty apply_paths, got {other:?}"),
    }
}

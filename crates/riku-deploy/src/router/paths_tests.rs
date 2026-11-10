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
                cert_for: None,
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

fn lock_is_free(paths: &RikuPaths) -> bool {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.riku_root.join("routing").join("lock"))
        .unwrap();
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    rc == 0
}

#[test]
fn the_routing_lock_is_held_while_the_path_site_is_sent() {
    let (_tmp, paths) = paths();
    set_mode(&paths, "shop", RoutingMode::Path);
    let free_during_send = RefCell::new(None);
    let send = |_: &Request| {
        *free_during_send.borrow_mut() = Some(lock_is_free(&paths));
        Ok(Reply::Ok { reloaded: true })
    };
    let lookup = |_: &str| Ok(vec![Backend::loopback(6000)]);

    publish_paths_with(
        send,
        lookup,
        &paths,
        &Router::Nginx,
        ("shop", vec![Backend::loopback(5001)]),
        false,
    )
    .unwrap();

    assert_eq!(
        *free_during_send.borrow(),
        Some(false),
        "another deploy could publish a stale list in between"
    );
}

#[test]
fn busy_shared_port_is_refused_before_the_path_site_exists() {
    let listening = std::collections::BTreeSet::from([8080]);

    let message = check_shared_port(8080, &listening, false)
        .unwrap_err()
        .to_string();

    assert!(message.contains("8080"), "{message}");
    assert!(
        check_shared_port(8080, &listening, true).is_ok(),
        "once the path site is up, the router itself listens there"
    );
    assert!(check_shared_port(8080, &Default::default(), false).is_ok());
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

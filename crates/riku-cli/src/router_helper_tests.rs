use std::path::Path;

use riku_route::ReloadMethod;
use tempfile::TempDir;

use super::*;

fn roots_in(dir: &Path) -> NginxRoots {
    NginxRoots {
        site_root: dir.join("etc/nginx"),
        cert_root: dir.join("etc/certs"),
        cache_root: dir.join("var/cache/nginx"),
        socket_root: dir.join("sockets"),
        static_root: dir.join("static"),
        acme_root: dir.join("acme"),
        cloudflare_conf: dir.join("etc/cloudflare.conf"),
        nginx: dir.join("no_nginx"),
        reload: ReloadMethod::Signal,
        owner_uid: 0,
    }
}

fn assert_failed_at(reply: &Reply, expected: Stage) {
    match reply {
        Reply::Failed { stage, .. } => assert_eq!(*stage, expected, "{reply:?}"),
        other => panic!("expected a failure at {expected:?}, got {other:?}"),
    }
}

#[test]
fn non_root_refuses_to_start() {
    let error = not_root(Uid::from_raw(1000)).expect("uid 1000 was allowed to start");

    assert!(error.to_string().contains("must run as root"), "{error}");
    assert!(not_root(Uid::from_raw(0)).is_none());
}

#[test]
fn bad_app_name_is_a_schema_failure() {
    let dir = TempDir::new().unwrap();

    let reply = handle(
        &roots_in(dir.path()),
        Request::Remove {
            app: "../etc".to_string(),
        },
    );

    assert_failed_at(&reply, Stage::Schema);
}

#[test]
fn remove_of_an_app_with_no_config_does_not_reload() {
    let dir = TempDir::new().unwrap();

    let reply = handle(
        &roots_in(dir.path()),
        Request::Remove {
            app: "myapp".to_string(),
        },
    );

    assert_eq!(reply, Reply::Ok { reloaded: false });
}

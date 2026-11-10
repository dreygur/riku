use std::net::{IpAddr, Ipv4Addr};

use super::*;

fn app(name: &str) -> PathApp {
    PathApp {
        app: name.to_string(),
        backends: vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5001,
        }],
    }
}

fn shared() -> Listen {
    Listen { port: 8080 }
}

#[test]
fn path_apps_pass_the_gate() {
    let checked = check_paths(&shared(), &[app("shop"), app("blog")]).unwrap();

    assert_eq!(checked.len(), 2);
}

#[test]
fn path_app_name_starting_with_underscore_is_refused() {
    let message = check_paths(&shared(), &[app("_paths")])
        .unwrap_err()
        .to_string();

    assert!(message.contains("_paths"), "{message}");
}

#[test]
fn duplicate_app_is_refused() {
    assert!(check_paths(&shared(), &[app("shop"), app("shop")]).is_err());
}

#[test]
fn privileged_shared_port_is_refused() {
    assert!(check_paths(&Listen { port: 80 }, &[app("shop")]).is_err());
}

#[test]
fn app_without_backend_or_with_remote_backend_is_refused() {
    let empty = PathApp {
        backends: Vec::new(),
        ..app("shop")
    };
    let remote = PathApp {
        backends: vec![RouteBackend::Tcp {
            host: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)),
            port: 5001,
        }],
        ..app("shop")
    };

    assert!(check_paths(&shared(), &[empty]).is_err());
    assert!(check_paths(&shared(), &[remote]).is_err());
}

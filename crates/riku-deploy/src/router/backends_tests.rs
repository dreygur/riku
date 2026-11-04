use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::mock_engine::mock_engine;
use crate::supervisor::config::create_worker_config;

fn paths_in(tmp: &TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    fs::create_dir_all(&paths.workers_available).unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

fn enable_worker(paths: &RikuPaths, app: &str, kind: &str, copy: u32, pairs: &[(&str, &str)]) {
    let env = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = create_worker_config(app, kind, "run", copy, env, "/srv/app", "/srv/log");
    let name = format!("{app}@{kind}.{copy}.toml");
    let file = paths.workers_enabled.join(name);
    fs::write(file, toml::to_string(&config).unwrap()).unwrap();
}

fn current(paths: &RikuPaths, app: &str) -> Result<Vec<Backend>> {
    current_backends(app, &HashMap::new(), paths, None)
}

#[test]
fn scaled_web_lists_every_copy() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    for copy in 1..=3 {
        let port = (5000 + copy).to_string();
        enable_worker(&paths, "myapp", "web", copy, &[("PORT", &port)]);
    }
    enable_worker(&paths, "other", "web", 1, &[("PORT", "6000")]);
    enable_worker(&paths, "myapp", "worker", 1, &[]);

    let backends = current(&paths, "myapp").unwrap();

    let expected: Vec<Backend> = (5001..=5003).map(Backend::loopback).collect();
    assert_eq!(backends, expected);
}

#[test]
fn wsgi_copies_get_own_socket() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    for copy in [1, 2] {
        let socket = copy_socket(&paths, "myapp", CopyIndex::new(copy).unwrap());
        enable_worker(
            &paths,
            "myapp",
            "wsgi",
            copy,
            &[("UWSGI_SOCKET", socket.to_str().unwrap())],
        );
    }

    let backends = current(&paths, "myapp").unwrap();

    let socket = |n: u32| paths.socket_root.join(format!("myapp@{n}.sock"));
    assert_eq!(
        backends,
        vec![Backend::Unix(socket(1)), Backend::Unix(socket(2))]
    );
}

#[test]
fn bind_address_of_the_worker_is_the_backend_host() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    enable_worker(
        &paths,
        "myapp",
        "web",
        1,
        &[("PORT", "5001"), ("RIKU_BIND_ADDRESS", "10.0.0.5")],
    );

    let backends = current(&paths, "myapp").unwrap();

    let host = "10.0.0.5".parse().unwrap();
    assert_eq!(backends, vec![Backend::Tcp { host, port: 5001 }]);
}

#[test]
fn app_without_web_copies_is_an_error_naming_it() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    enable_worker(&paths, "myapp", "worker", 1, &[]);

    let err = current(&paths, "myapp").unwrap_err();

    assert!(err.to_string().contains("myapp"), "{err}");
}

fn container_json(id: &str, copy: u32, port: Option<u16>, status: &str) -> String {
    let port_label = port
        .map(|p| format!(r#","riku.port":"{p}""#))
        .unwrap_or_default();
    format!(
        r#"{{"Id":"{id}","Name":"/myapp.web.{copy}","Image":"sha256:x","Config":{{"Image":"shop/web:1","Labels":{{"riku.app":"myapp","riku.service":"web","riku.copy":"{copy}"{port_label}}}}},"State":{{"Status":"{status}","ExitCode":0}}}}"#
    )
}

#[test]
fn container_app_routes_running_copies_by_published_port() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    let ids: Vec<String> = ('a'..='d').map(|c| c.to_string().repeat(64)).collect();
    let inspected = [
        container_json(&ids[1], 2, Some(32002), "running"),
        container_json(&ids[0], 1, Some(32001), "running"),
        container_json(&ids[2], 3, Some(32003), "exited"),
        container_json(&ids[3], 4, None, "running"),
    ]
    .join(",");
    let behaviour = format!(
        "case \"$1\" in ps) printf '%s\\n' {};; inspect) echo '[{inspected}]';; esac",
        ids.join(" ")
    );
    let (engine, _log) = mock_engine(Path::new(tmp.path()), &behaviour);

    let backends = current_backends("myapp", &HashMap::new(), &paths, Some(&engine)).unwrap();

    assert_eq!(
        backends,
        vec![Backend::loopback(32001), Backend::loopback(32002)]
    );
}

#[test]
fn bind_address_that_is_not_an_ip_is_refused_naming_the_key() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_in(&tmp);
    enable_worker(
        &paths,
        "myapp",
        "web",
        1,
        &[("PORT", "5001"), ("RIKU_BIND_ADDRESS", "localhost")],
    );

    let err = format!("{:#}", current(&paths, "myapp").unwrap_err());

    assert!(err.contains("RIKU_BIND_ADDRESS"), "{err}");
}

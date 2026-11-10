use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::*;
use crate::router::tests::env;
use crate::util::domain::parse_domains;
use crate::util::route_plan::HttpsPolicy;

fn write_router_bundle(paths: &RikuPaths, name: &str, router_table: &str, script: &str) -> PathBuf {
    let bundle = paths.plugin_root.join(name);
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    std::fs::write(
        bundle.join("riku-plugin.toml"),
        format!(
            "name = \"{name}\"\nversion = \"0.1.0\"\ntype = \"router\"\napi = 1\nentry = \"bin/router\"\n{router_table}\n"
        ),
    )
    .unwrap();
    let entry = bundle.join("bin/router");
    std::fs::write(&entry, script).unwrap();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
    bundle
}

#[test]
fn configure_request_lists_every_backend() {
    let plan = RoutePlan {
        domains: parse_domains("shop.example.com").unwrap(),
        https: HttpsPolicy::Only,
        backends: vec![
            Backend::loopback(5001),
            Backend::Unix(PathBuf::from("/srv/riku/nginx/shop.sock")),
        ],
        open_port: None,
    };

    assert_eq!(
        configure_request("shop", &plan),
        serde_json::json!({
            "protocol": 2,
            "app": "shop",
            "domains": ["shop.example.com"],
            "https": true,
            "backends": [
                {"kind": "tcp", "host": "127.0.0.1", "port": 5001},
                {"kind": "unix", "path": "/srv/riku/nginx/shop.sock"},
            ],
        })
    );
}

#[test]
fn old_router_plugin_is_refused_before_running() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let ran = tmp.path().join("ran");
    let script = format!("#!/bin/sh\ntouch {}\n", ran.display());
    write_router_bundle(&paths, "edge-router", "", &script);

    let err = configure_plugin(
        "edge-router",
        "shop",
        &env(&[]),
        &[Backend::loopback(5001)],
        &paths,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("router protocol 1"), "{err}");
    assert!(err.contains("needs protocol 2"), "{err}");
    assert!(!ran.exists(), "plugin ran before the protocol check");
}

#[test]
fn router_plugin_on_another_protocol_is_refused_with_its_number() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    write_router_bundle(
        &paths,
        "edge-router",
        "[router]\nprotocol = 3",
        "#!/bin/sh\n",
    );

    let err = configure_plugin(
        "edge-router",
        "shop",
        &env(&[]),
        &[Backend::loopback(5001)],
        &paths,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("router protocol 3"), "{err}");
}

#[test]
fn protocol_2_plugin_gets_the_request_and_reload_is_a_separate_call() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let request = tmp.path().join("request.json");
    let verbs = tmp.path().join("verbs");
    let script = format!(
        "#!/bin/sh\necho \"$1\" >> {verbs}\nif [ \"$1\" = configure ]; then cat > {request}; fi\n",
        verbs = verbs.display(),
        request = request.display(),
    );
    write_router_bundle(&paths, "edge-router", "[router]\nprotocol = 2", &script);
    let app_env = env(&[("RIKU_DOMAINS", "shop.example.com"), ("PORT", "5001")]);

    configure_plugin(
        "edge-router",
        "shop",
        &app_env,
        &[Backend::loopback(5001)],
        &paths,
    )
    .unwrap();

    let sent: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&request).unwrap()).unwrap();
    assert_eq!(sent["protocol"], 2, "{sent}");
    assert_eq!(sent["domains"], serde_json::json!(["shop.example.com"]));
    assert_eq!(
        sent["backends"],
        serde_json::json!([{"kind": "tcp", "host": "127.0.0.1", "port": 5001}])
    );
    assert_eq!(std::fs::read_to_string(&verbs).unwrap(), "configure\n");
    reload_plugin("edge-router", &paths).unwrap();
    assert_eq!(
        std::fs::read_to_string(&verbs).unwrap(),
        "configure\nreload\n"
    );
}

#[test]
fn unconfigure_still_runs_for_an_old_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let verbs = tmp.path().join("verbs");
    let script = format!("#!/bin/sh\necho \"$1\" >> {}\n", verbs.display());
    write_router_bundle(&paths, "edge-router", "", &script);

    remove_plugin("edge-router", "shop", &paths).unwrap();

    assert_eq!(
        std::fs::read_to_string(&verbs).unwrap(),
        "unconfigure\nreload\n"
    );
}

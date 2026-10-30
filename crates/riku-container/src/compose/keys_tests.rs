use super::*;
use serde_json::json;

fn refusal(project: serde_json::Value) -> String {
    let project = project.as_object().unwrap().clone();
    check_keys(&project)
        .expect_err("project should be refused")
        .to_string()
}

#[test]
fn supported_top_level_keys_and_default_network_pass() {
    let project = json!({
        "version": "3.8",
        "name": "shop",
        "x-common": {"anything": 1},
        "networks": {"default": {}},
        "volumes": {"data": null},
        "services": {"web": {"image": "nginx", "ports": ["80:80"]}},
    });
    assert!(check_keys(project.as_object().unwrap()).is_ok());
}

#[test]
fn unknown_service_key_names_service_and_key() {
    let err = refusal(json!({"services": {"worker": {"image": "x", "deploy": {"replicas": 2}}}}));
    assert_eq!(err, "service 'worker': unsupported key 'deploy'");
}

#[test]
fn unknown_top_level_key_is_refused() {
    let err = refusal(json!({"services": {}, "secrets": {}}));
    assert_eq!(err, "unsupported top level key 'secrets'");
}

#[test]
fn only_a_bare_default_network_is_allowed() {
    let err = refusal(json!({"networks": {"backend": {}}}));
    assert!(err.contains("backend"), "{err}");

    let err = refusal(json!({"networks": {"default": {"driver": "bridge"}}}));
    assert!(err.contains("default"), "{err}");
}

use super::build_app_metrics_json;

#[test]
fn unknown_app_name_with_quotes_stays_valid_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let stats_file = dir.path().join("stats.json");
    std::fs::write(&stats_file, "[]").unwrap();

    let name = r#"x","error":"forged"#;
    let raw = build_app_metrics_json(&stats_file, Some(name));
    let body: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => panic!("not-found body is not JSON ({e}): {raw}"),
    };

    assert_eq!(body["error"], "Not Found", "body: {raw}");
    assert_eq!(body["app"], name, "body: {raw}");
}

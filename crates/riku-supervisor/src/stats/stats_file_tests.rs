use super::*;

/// A file that does not parse is left for the supervisor to rewrite, not
/// replaced by one holding a single app.
#[test]
fn an_unparsable_stats_file_is_not_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let stats = tmp.path().join("stats.json");
    fs::write(&stats, "{ half a write").unwrap();

    let result = replace_app_container_rows(&stats, "shop", Vec::new());

    assert!(result.is_err(), "an unparsable file was replaced");
    assert_eq!(fs::read_to_string(&stats).unwrap(), "{ half a write");
}

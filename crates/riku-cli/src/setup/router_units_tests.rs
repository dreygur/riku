use super::*;

#[test]
fn plugin_router_orders_after_the_network_only() {
    assert_eq!(
        unit_ordering(&Router::Plugin("traefik".to_string())),
        "After=network.target"
    );
}

#[test]
fn old_watcher_units_are_removed_and_a_second_run_finds_none() {
    let tmp = tempfile::tempdir().unwrap();
    for name in STALE_UNITS {
        fs::write(tmp.path().join(name), "[Unit]\n").unwrap();
    }
    fs::write(tmp.path().join("riku.service"), "[Unit]\n").unwrap();

    assert!(remove_stale_units(tmp.path()).unwrap());
    assert!(!remove_stale_units(tmp.path()).unwrap());

    assert!(tmp.path().join("riku.service").exists());
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
}

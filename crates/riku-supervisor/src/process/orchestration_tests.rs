use crate::process::test_support::minimal_config;
use crate::process::ProcessManager;

#[test]
fn removing_a_config_stops_its_probing_canary() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("out.log");
    let mut config = minimal_config(
        "sleep 60",
        tmp.path().to_str().unwrap(),
        log.to_str().unwrap(),
    );
    config.options.grace_period = 1;
    // Nothing listens on port 1, so the probe never promotes the canary.
    config.env.insert("PORT".to_string(), "1".to_string());
    let process_id = config.worker.name().to_string();

    let mut pm = ProcessManager::new().unwrap();
    pm.deploy_generation(&process_id, config)
        .expect("deploy canary");
    let canary = pm.list_processes();
    assert_eq!(canary.len(), 1, "the canary should be running: {canary:?}");

    pm.stop_canaries(&process_id).expect("stop canaries");

    assert_eq!(
        pm.get_process_count(),
        0,
        "canary {} still managed",
        canary[0].process_id
    );
    assert!(
        !pm.generations.contains_key(&process_id),
        "generations of a removed config must be dropped"
    );
}

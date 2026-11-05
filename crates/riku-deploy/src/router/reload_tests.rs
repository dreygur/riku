use super::*;

#[test]
fn a_failure_is_recorded_for_doctor_and_a_confirmed_reload_clears_it() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let file = paths.riku_root.join(ERROR_FILE);

    record_outcome(&paths, &Err(anyhow::anyhow!("admin endpoint disabled")));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "admin endpoint disabled"
    );

    record_outcome(&paths, &Ok(()));
    assert!(!file.exists(), "failure text stayed after a good reload");
}

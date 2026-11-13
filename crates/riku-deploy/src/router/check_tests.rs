use std::collections::HashMap;

use super::*;

#[test]
fn path_mode_app_passes_the_nginx_check_without_a_domain() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[router]\nname = \"nginx\"\n").unwrap();
    let env = HashMap::from([("RIKU_ROUTING_MODE".to_string(), "path".to_string())]);

    let checked = check("shop", &env, &paths);

    assert!(
        checked.is_ok(),
        "a path mode app has no domain site to check: {checked:?}"
    );
}

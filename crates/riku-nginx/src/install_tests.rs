use std::collections::HashMap;
use std::fs;

use crate::test_support::{paths_with_nginx, REJECT};
use crate::{default_backends, generate_nginx_config};

#[test]
fn a_rejected_config_leaves_the_live_one_untouched() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), REJECT);
    let live = paths.nginx_root.join("myapp.conf");
    fs::write(&live, "# previous\n").unwrap();
    let env = HashMap::from([("RIKU_DOMAINS".to_string(), "example.com".to_string())]);

    let outcome = generate_nginx_config(
        "myapp",
        temp_dir.path(),
        &env,
        &default_backends("myapp", &env, &paths).unwrap(),
        &paths,
    );

    let err = outcome.expect_err("nginx -t failed but the config was accepted");
    assert!(err.to_string().contains("unexpected end of file"), "{err}");
    assert_eq!(fs::read_to_string(&live).unwrap(), "# previous\n");
    let left: Vec<_> = fs::read_dir(&paths.nginx_root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, vec!["myapp.conf"], "staging files were left behind");
}

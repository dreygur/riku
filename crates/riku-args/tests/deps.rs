//! Guards the reason this crate exists: a client binary built on it must not
//! link any server side crate or the HTTP stack.

use std::process::Command;

const FORBIDDEN: [&str; 8] = [
    "riku-deploy",
    "riku-supervisor",
    "riku-nginx",
    "riku-caddy",
    "riku-plugins",
    "riku-dashboard",
    "riku-config",
    "reqwest",
];

#[test]
fn depends_on_no_server_crate() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "riku-args",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree should run");
    assert!(output.status.success(), "cargo tree failed: {output:?}");

    let tree = String::from_utf8(output.stdout).expect("cargo tree output is utf-8");
    let leaked: Vec<&str> = tree
        .lines()
        .filter(|line| FORBIDDEN.iter().any(|name| line.starts_with(name)))
        .collect();
    assert!(leaked.is_empty(), "forbidden dependencies: {leaked:?}");
}

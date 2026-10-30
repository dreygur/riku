//! Guards why riku-client and riku-args exist: neither may link a server side
//! crate or the HTTP stack. riku-remote is covered through riku-client.

use std::process::Command;

const FORBIDDEN: [&str; 8] = [
    "riku-deploy",
    "riku-supervisor",
    "riku-nginx",
    "riku-caddy",
    "riku-plugins",
    "riku-dashboard",
    "riku-cli",
    "reqwest",
];

fn leaked(package: &str, forbidden: &[&str]) -> Vec<String> {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            package,
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--offline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree should run");
    assert!(output.status.success(), "cargo tree failed: {output:?}");

    let tree = String::from_utf8(output.stdout).expect("cargo tree output is utf-8");
    tree.lines()
        .filter(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|name| forbidden.contains(&name))
        })
        .map(|line| format!("{package}: {line}"))
        .collect()
}

#[test]
fn client_crates_link_no_server_crate() {
    let mut args_forbidden = FORBIDDEN.to_vec();
    args_forbidden.push("riku-config");
    let checks = [
        ("riku-client", &FORBIDDEN[..]),
        ("riku-args", &args_forbidden[..]),
    ];
    let leaks: Vec<String> = checks
        .iter()
        .flat_map(|(package, forbidden)| leaked(package, forbidden))
        .collect();
    assert!(leaks.is_empty(), "forbidden dependencies: {leaks:?}");
}

//! Forwarding through the built binary.
//!
//! The server address uses the reserved `.invalid` domain, so ssh fails at
//! the name lookup and no machine is ever contacted. The config directory is
//! set on the child process only.

use std::process::{Command, Stdio};

#[test]
fn forwarded_command_names_its_target_and_fails_with_ssh() {
    let config = tempfile::tempdir().unwrap();
    let riku_dir = config.path().join("riku");
    std::fs::create_dir(&riku_dir).unwrap();
    std::fs::write(
        riku_dir.join("servers.toml"),
        "[[server]]\nname = \"ghost\"\naddress = \"riku@nonexistent.invalid\"\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_riku"))
        .args(["-s", "ghost", "ps"])
        .env("XDG_CONFIG_HOME", config.path())
        .env_remove("SSH_ORIGINAL_COMMAND")
        .current_dir(config.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[*] ghost (riku@nonexistent.invalid)"),
        "target line missing from stderr:\n{stderr}"
    );
    assert!(
        !out.status.success(),
        "ssh to an .invalid host should fail, got {:?}:\n{stderr}",
        out.status
    );
}

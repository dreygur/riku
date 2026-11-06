use super::*;
use crate::doctor::Status;

const FOREIGN_KEY: &str = "ssh-ed25519 AAAAC3Nza foreign@laptop  ";
const FOREIGN_COMMAND: &str = "command=\"/usr/bin/true\",no-pty ssh-rsa AAAAB3 other";

fn riku_key(program: &str) -> String {
    format!("command=\"FINGERPRINT=fp1 NAME=default {program}\",no-agent-forwarding ssh-ed25519 AAAA me")
}

#[test]
fn rewrites_a_stale_key_and_leaves_other_lines_byte_for_byte() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = tmp.path().join("authorized_keys");
    let before = format!(
        "{FOREIGN_KEY}\n{}\n{FOREIGN_COMMAND}\n{}",
        riku_key("/home/deploy/.local/bin/riku"),
        riku_key(ROOT_BINARY)
    );
    fs::write(&keys, &before).unwrap();

    let checks = fix_stale_launch_paths(&keys, &tmp.path().join("repos"));

    let expected = before.replace("/home/deploy/.local/bin/riku", ROOT_BINARY);
    assert_eq!(fs::read_to_string(&keys).unwrap(), expected);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Ok, "{}", checks[0].detail);
}

#[test]
fn rewrites_a_stale_hook_and_keeps_a_current_and_a_foreign_one() {
    let tmp = tempfile::tempdir().unwrap();
    let repos = tmp.path().join("repos");
    let stale = "#!/bin/bash\nexec /opt/old/riku git-hook \"$APP\"\n";
    let foreign = "#!/bin/sh\necho notify\n";
    for (repo, body) in [
        ("old.git", stale.to_string()),
        ("mine.git", post_receive_hook()),
        ("other.git", foreign.to_string()),
    ] {
        let hooks = repos.join(repo).join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("post-receive"), body).unwrap();
    }

    let checks = fix_stale_launch_paths(&tmp.path().join("authorized_keys"), &repos);

    let rewritten = repos.join("old.git/hooks/post-receive");
    assert_eq!(fs::read_to_string(&rewritten).unwrap(), post_receive_hook());
    assert_eq!(
        fs::metadata(&rewritten).unwrap().permissions().mode() & 0o777,
        HOOK_MODE
    );
    assert_eq!(
        fs::read_to_string(repos.join("other.git/hooks/post-receive")).unwrap(),
        foreign
    );
    assert_eq!(checks.len(), 1);
    assert!(checks[0].detail.contains("old.git"), "{}", checks[0].detail);
    assert!(
        !checks[0].detail.contains("mine.git"),
        "{}",
        checks[0].detail
    );
}

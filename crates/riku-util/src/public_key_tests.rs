use super::*;

#[test]
fn keeps_only_type_and_blob() {
    let text = "\
# laptop
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample me@laptop

command=\"echo ssh-rsa FAKE\",no-pty ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB other
";
    let keys = parse_public_keys(text).unwrap();
    let lines: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(
        lines,
        [
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample",
            "ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB",
        ],
        "options, comments and quoted decoys must not survive"
    );
}

#[test]
fn rejects_a_line_that_is_not_a_key() {
    let err = parse_public_keys("ssh-ed25519 AAAAC3Nza\nroot:x:0:0:root:/root:/bin/bash\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("line 2"), "error should name the line: {err}");

    let err = parse_public_keys("ssh-ed25519 AAAA\"injected\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("no valid key data"), "{err}");

    assert!(
        parse_public_keys("# only a comment\n").is_err(),
        "an empty file must fail"
    );
}

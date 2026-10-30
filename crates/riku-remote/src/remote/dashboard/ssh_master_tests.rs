use super::*;

fn sv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn every_step_reaches_the_admin_through_one_control_socket() {
    let admin = SshAddress::try_from("admin@edge.example").unwrap();
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    let ssh = SshControl::new(&ctl, &admin);
    let socket = ["-S", "/tmp/riku-dash/%C"];
    let host = ["--", "admin@edge.example"];
    let join = |parts: &[&[&str]]| sv(&parts.concat());

    let master = [
        "-M",
        "-o",
        "ControlPersist=no",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=3",
        "-N",
    ];
    assert_eq!(ssh.master_args(), join(&[&socket, &master, &host]));
    assert_eq!(ssh.check_args(), join(&[&socket, &["-O", "check"], &host]));
    assert_eq!(
        ssh.forward_args(8088, 43121),
        join(&[
            &socket,
            &["-O", "forward", "-L", "127.0.0.1:8088:127.0.0.1:43121"],
            &host
        ])
    );
    assert_eq!(ssh.exit_args(), join(&[&socket, &["-O", "exit"], &host]));
}

#[test]
fn remote_command_is_one_quoted_argument() {
    let admin = SshAddress::try_from("admin@edge.example").unwrap();
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    let words = sv(&["sh", "-c", "echo 'a b' \"$1\"", "x"]);
    let args = SshControl::new(&ctl, &admin).command_args(&words);
    let remote = args.last().unwrap();
    assert_eq!(args[args.len() - 2], "admin@edge.example");
    assert_eq!(
        shlex::split(remote),
        Some(words),
        "remote command {remote:?}"
    );
}

#[test]
fn control_path_uses_ssh_hash_token() {
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    assert!(
        ctl.to_string_lossy().ends_with("/%C"),
        "control path {ctl:?} should end with /%C"
    );
}

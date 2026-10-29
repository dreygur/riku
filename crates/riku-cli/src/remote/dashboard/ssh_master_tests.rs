use super::*;
use crate::remote::dashboard::remote_script::probe_command;

fn sv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn every_step_reaches_the_admin_through_one_control_socket() {
    let admin = SshAddress::try_from("dreygur@solutech.local").unwrap();
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    let ssh = SshControl::new(&ctl, &admin);
    let socket = ["-S", "/tmp/riku-dash/%C"];
    let host = ["--", "dreygur@solutech.local"];
    let join = |parts: &[&[&str]]| sv(&parts.concat());

    assert_eq!(
        ssh.master_args(),
        join(&[&socket, &["-M", "-o", "ControlPersist=yes", "-N"], &host])
    );
    assert_eq!(ssh.check_args(), join(&[&socket, &["-O", "check"], &host]));
    assert_eq!(
        ssh.command_args(&probe_command("riku")),
        join(&[&socket, &host, &["sudo -n -u riku true"]])
    );
    assert_eq!(
        ssh.forward_args(8088, 43121),
        join(&[
            &socket,
            &["-O", "forward", "-L", "8088:127.0.0.1:43121"],
            &host
        ])
    );
    assert_eq!(ssh.exit_args(), join(&[&socket, &["-O", "exit"], &host]));
}

#[test]
fn remote_command_is_one_quoted_argument() {
    let admin = SshAddress::try_from("dreygur@solutech.local").unwrap();
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    let words = sv(&["sh", "-c", "echo 'a b' \"$1\"", "x"]);
    let args = SshControl::new(&ctl, &admin).command_args(&words);
    let remote = args.last().unwrap();
    assert_eq!(args[args.len() - 2], "dreygur@solutech.local");
    assert_eq!(
        shlex::split(remote),
        Some(words),
        "remote command {remote:?}"
    );
}

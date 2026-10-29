use std::path::Path;

use super::*;
use crate::remote::dashboard::ssh_master::{control_path, SshControl};
use crate::remote::SshAddress;

const TOKEN: &str = "5f1c0ffee5f1c0ffee5f1c0ffee5f1c0ffee5f1c0ffee5f1c0ffee5f1c0ffee5";
const PASSWORD: &str = "correct horse";

#[test]
fn payload_without_password_is_token_line() {
    assert_eq!(payload(None, "abc"), "abc\n");
}

#[test]
fn payload_with_password_puts_it_first() {
    assert_eq!(payload(Some("pw"), "abc"), "pw\nabc\n");
}

#[test]
fn token_never_appears_in_ssh_arguments() {
    let admin = SshAddress::try_from("dreygur@solutech.local").unwrap();
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    let ssh = SshControl::new(&ctl, &admin);
    let argvs = [
        ssh.master_args(),
        ssh.check_args(),
        ssh.command_args(&probe_command("riku")),
        ssh.command_args(&script_command(SudoMode::NoPassword, "riku")),
        ssh.command_args(&script_command(SudoMode::Password, "riku")),
        ssh.forward_args(8088, 43121),
        ssh.exit_args(),
    ];
    for word in argvs.iter().flatten() {
        assert!(
            !word.contains(TOKEN),
            "token leaked into ssh argument {word:?}"
        );
        assert!(
            !word.contains(PASSWORD),
            "password leaked into ssh argument {word:?}"
        );
    }
    assert!(payload(Some(PASSWORD), TOKEN).contains(TOKEN));
}

#[test]
fn deploy_user_is_passed_as_argument_not_spliced() {
    let words = script_command(SudoMode::NoPassword, "deployer");
    assert_eq!(words.last().map(String::as_str), Some("deployer"));
    assert_eq!(words[..2], ["sh".to_string(), "-c".to_string()]);
    let text = &words[2];
    assert!(
        text.contains("-u \"$1\""),
        "script should read the user from $1: {text}"
    );
    assert!(
        !text.contains("deployer"),
        "user spliced into script: {text}"
    );
}

#[test]
fn script_matches_the_pinned_text_for_each_sudo_mode() {
    let body = " -H -u \"$1\" sh -c '
  IFS= read -r t
  export RIKU_DASHBOARD_TOKEN=\"$t\"
  bin=$(command -v riku || echo \"$HOME/.local/bin/riku\")
  \"$bin\" dashboard --bind 127.0.0.1:0 &
  p=$!
  cat >/dev/null
  kill \"$p\"
'";
    assert_eq!(script(SudoMode::NoPassword), format!("sudo -n{body}"));
    assert_eq!(script(SudoMode::Password), format!("sudo -S -p ''{body}"));
}

#[test]
fn asks_for_a_password_only_when_sudo_says_one_is_required() {
    assert!(needs_password("sudo: a password is required\n"));
    assert!(!needs_password(
        "dreygur is not in the sudoers file.  This incident will be reported.\n"
    ));
}

#[test]
fn parses_bound_port_line() {
    assert_eq!(parse_bound("riku-dashboard-bound 43121"), Some(43121));
    assert_eq!(parse_bound("-----> Dashboard listening"), None);
}

#[test]
fn control_path_uses_ssh_hash_token() {
    let ctl = control_path(Path::new("/tmp/riku-dash"));
    assert!(
        ctl.to_string_lossy().ends_with("/%C"),
        "control path {ctl:?} should end with /%C"
    );
}

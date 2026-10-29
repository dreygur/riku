use super::*;
use crate::remote::{ServerName, SshAddress};

fn sv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn solutech() -> Server {
    Server {
        name: ServerName::try_from("solutech").unwrap(),
        address: SshAddress::try_from("riku@solutech.local").unwrap(),
        admin: None,
    }
}

#[test]
fn strips_only_consumed_server_tokens() {
    let argv = sv(&["riku", "-s", "prod", "run", "myapp", "grep", "-s", "x"]);
    assert_eq!(
        strip_target_args(&argv, &[1, 2]),
        sv(&["run", "myapp", "grep", "-s", "x"])
    );
}

#[test]
fn finds_the_server_option_clap_consumed() {
    let cli = crate::help::command("Forwarded commands:");
    let positions = |words: &[&str]| server_arg_positions(&cli, &sv(words)).unwrap();
    assert_eq!(
        positions(&["riku", "-s", "prod", "run", "myapp", "grep", "-s", "prod"]),
        [1, 2]
    );
    assert_eq!(positions(&["riku", "--server=prod", "ps", "x"]), [1]);
    assert_eq!(positions(&["riku", "ps", "x", "-s", "prod"]), [3, 4]);
    assert_eq!(
        positions(&["riku", "run", "myapp", "grep", "-s", "x"]),
        Vec::<usize>::new(),
        "a -s inside the remote command must stay"
    );
}

#[test]
fn ssh_args_quote_each_argument_and_skip_riku_word() {
    let a = ssh_args(
        &solutech(),
        &sv(&["run", "myapp", "echo", "a b"]),
        true,
        None,
    );
    assert_eq!(
        a,
        sv(&["-t", "--", "riku@solutech.local", "run myapp echo 'a b'"])
    );
}

#[test]
fn app_hint_added_only_when_present() {
    let with = ssh_args(&solutech(), &sv(&["logs"]), false, Some("shop"));
    assert_eq!(
        with,
        sv(&["--", "riku@solutech.local", "--app-hint shop logs"])
    );
    let without = ssh_args(&solutech(), &sv(&["logs"]), false, None);
    assert!(
        !without.last().unwrap().contains("--app-hint"),
        "{without:?}"
    );
}

#[test]
fn tty_only_for_run_and_follow() {
    assert!(needs_tty(&sv(&["run", "myapp", "bash"])));
    assert!(needs_tty(&sv(&["logs", "myapp", "-d", "-f"])));
    assert!(needs_tty(&sv(&["logs", "myapp", "-df"])));
    assert!(!needs_tty(&sv(&["logs", "myapp"])));
    assert!(!needs_tty(&sv(&["ps"])));
}

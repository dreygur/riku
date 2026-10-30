use super::*;

#[test]
fn deploy_user_is_passed_as_argument_not_spliced() {
    let words = script_command("deployer");
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
fn script_prompts_through_sudo_and_reads_the_token_after_ready() {
    let expected = "sudo -S -p 'riku-sudo-prompt:' -H -u \"$1\" sh -c '
  echo riku-ready
  IFS= read -r t || exit 1
  export RIKU_DASHBOARD_TOKEN=\"$t\"
  bin=$(command -v riku || echo \"$HOME/.local/bin/riku\")
  \"$bin\" dashboard --local --bind 127.0.0.1:0 &
  p=$!
  cat >/dev/null
  kill \"$p\"
'";
    assert_eq!(script(), expected);
}

#[test]
fn parses_bound_port_line() {
    assert_eq!(parse_bound("riku-dashboard-bound 43121"), Some(43121));
    assert_eq!(parse_bound("-----> Dashboard listening"), None);
}

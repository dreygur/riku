use super::*;

fn shown(actions: &[Action]) -> Vec<u8> {
    actions
        .iter()
        .flat_map(|action| match action {
            Action::ShowStderr(bytes) => bytes.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn asks_for_the_password_when_the_prompt_arrives_split_across_reads() {
    let mut hs = Handshake::new();
    let first = hs.stderr(b"lecture text\nriku-sudo");
    assert_eq!(shown(&first), b"lecture text\n".to_vec());
    assert!(
        !first.contains(&Action::AskPassword),
        "asked too early: {first:?}"
    );
    assert_eq!(hs.stderr(b"-prompt:"), vec![Action::AskPassword]);
}

#[test]
fn second_prompt_means_sudo_rejected_the_password() {
    let mut hs = Handshake::new();
    assert_eq!(hs.stderr(b"riku-sudo-prompt:"), vec![Action::AskPassword]);
    let retry = hs.stderr(b"Sorry, try again.\nriku-sudo-prompt:");
    assert_eq!(retry.last(), Some(&Action::RejectPassword), "{retry:?}");
    assert!(
        !retry.contains(&Action::AskPassword),
        "asked twice: {retry:?}"
    );
}

#[test]
fn ready_without_a_prompt_sends_the_token_then_reads_the_port() {
    let mut hs = Handshake::new();
    assert_eq!(hs.stdout_line("riku-ready\n"), vec![Action::SendToken]);
    assert_eq!(
        hs.stdout_line("-----> Dashboard listening\n"),
        vec![Action::ShowStdout("-----> Dashboard listening".to_string())]
    );
    assert_eq!(
        hs.stdout_line("riku-dashboard-bound 43121\n"),
        vec![Action::Bound(43121)]
    );
}

#[test]
fn never_sends_the_token_before_the_ready_marker() {
    let mut hs = Handshake::new();
    let mut actions = hs.stderr(b"riku-sudo-prompt:");
    actions.extend(hs.stdout_line("riku-dashboard-bound 43121\n"));
    actions.extend(hs.stdout_line("riku-ready-not\n"));
    actions.extend(hs.stderr(b"riku-ready\n"));
    assert!(!actions.contains(&Action::SendToken), "{actions:?}");
    assert!(
        !actions.iter().any(|a| matches!(a, Action::Bound(_))),
        "{actions:?}"
    );
    assert_eq!(hs.stdout_line("riku-ready\n"), vec![Action::SendToken]);
}

#[test]
fn held_prompt_prefix_is_shown_once_it_cannot_be_the_prompt() {
    let mut hs = Handshake::new();
    assert_eq!(
        shown(&hs.stderr(b"ssh: warning riku")),
        b"ssh: warning ".to_vec()
    );
    assert_eq!(shown(&hs.stderr(b" more\n")), b"riku more\n".to_vec());
    let _ = hs.stderr(b"tail r");
    assert_eq!(shown(&hs.stderr_closed()), b"r".to_vec());
}

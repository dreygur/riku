use super::*;

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}

#[test]
fn forced_command_with_only_scope_and_identity_takes_a_request() {
    assert!(is_bare_agent(&words(
        "riku agent --scope staging --agent-id cursor"
    )));
    assert!(!is_bare_agent(&words("riku agent --scope staging apps")));
    assert!(!is_bare_agent(&words("riku ps")));
}

#[test]
fn client_request_keeps_command_arguments_and_confirm_token() {
    let request = client_request(words("agent destroy shop --confirm abc"));
    assert_eq!(request.unwrap(), words("destroy shop --confirm abc"));
}

#[test]
fn client_cannot_choose_scope_identity_or_target() {
    for sneaky in [
        "deploy shop --scope production",
        "deploy shop --scope=production",
        "apps --agent-id admin",
        "apps -s other",
        "apps --local",
    ] {
        assert!(
            client_request(words(sneaky)).is_err(),
            "accepted a client request that sets a forced-only option: {sneaky}"
        );
    }
}

#[test]
fn words_after_separator_stay_positional() {
    let request = client_request(words("run shop -- ls -la"));
    assert_eq!(request.unwrap(), words("run shop -- ls -la"));
}

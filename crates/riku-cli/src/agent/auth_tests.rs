use super::*;

#[test]
fn session_defaults_to_readonly_and_unknown_agent() {
    let session = AgentSession::from_flags(None, None);
    assert_eq!(session.scope, AgentScope::Readonly);
    assert_eq!(session.id, "unknown-agent");
}

#[test]
fn agent_id_that_could_leave_the_rate_directory_is_replaced() {
    for bad in ["../etc/x", "a/b", ".hidden", ""] {
        let session = AgentSession::from_flags(Some("staging"), Some(bad));
        assert_eq!(session.id, "unknown-agent", "kept {bad:?}");
        assert_eq!(session.scope, AgentScope::Staging);
    }
}

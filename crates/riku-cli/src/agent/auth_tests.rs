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

fn state_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::TempDir::new().unwrap();
    let dir = super::super::state::agent_dir(root.path()).unwrap();
    (root, dir)
}

#[test]
fn rate_limit_blocks_after_the_scope_limit_and_reopens_next_window() {
    let (_root, dir) = state_dir();
    let limit = AgentScope::Production.rate_limit();
    for n in 0..limit {
        assert!(
            check_rate_limit(&dir, "a", &AgentScope::Production, 1000).unwrap(),
            "{n}"
        );
    }
    assert!(!check_rate_limit(&dir, "a", &AgentScope::Production, 1000).unwrap());
    assert!(check_rate_limit(&dir, "a", &AgentScope::Production, 1000 + RATE_WINDOW_SECS).unwrap());
}

#[test]
fn planted_future_timestamp_neither_panics_nor_lifts_the_limit() {
    let (_root, dir) = state_dir();
    let planted = vec!["99999999999\n"; AgentScope::Production.rate_limit() as usize].concat();
    std::fs::write(dir.join("rate-a.log"), planted).unwrap();
    assert!(!check_rate_limit(&dir, "a", &AgentScope::Production, 1000).unwrap());
    assert!(check_rate_limit(&dir, "a", &AgentScope::Production, 1000 + RATE_WINDOW_SECS).unwrap());
}

#[test]
fn symlinked_rate_file_is_an_error_not_a_pass() {
    let (root, dir) = state_dir();
    let victim = root.path().join("victim");
    std::fs::write(&victim, "keep").unwrap();
    std::os::unix::fs::symlink(&victim, dir.join("rate-a.log")).unwrap();
    assert!(check_rate_limit(&dir, "a", &AgentScope::Readonly, 1000).is_err());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
}

#[test]
fn audit_line_is_appended_to_the_private_log() {
    let (_root, dir) = state_dir();
    log_agent_action(&dir, "cursor", "deploy", "web", true);
    let log = std::fs::read_to_string(dir.join(AUDIT_FILE)).unwrap();
    assert!(log.contains("[AI] cursor deploy web success"), "{log}");
}

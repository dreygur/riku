use tempfile::TempDir;

use super::*;
use crate::agent::types::AgentScope;

#[test]
fn mutating_command_is_refused_without_a_writable_audit_log() {
    let root = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(root.path());
    let state = agent_dir(&paths.riku_root).unwrap();
    let victim = root.path().join("victim");
    std::fs::write(&victim, "keep").unwrap();
    std::os::unix::fs::symlink(&victim, state.join("audit.log")).unwrap();
    let session = AgentSession {
        id: "mock_agent".to_string(),
        scope: AgentScope::Production,
    };
    let args = ["myapp".to_string(), "KEY=value".to_string()];

    let response = run_agent_command(&paths, &session, "config:set", &args, None);

    let code = response.error.map(|e| e.code);
    assert_eq!(code.as_deref(), Some("STATE_UNAVAILABLE"));
    assert!(!paths.env_root.join("myapp").join("ENV").exists());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
}

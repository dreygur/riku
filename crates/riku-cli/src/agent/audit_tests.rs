use tempfile::TempDir;

use super::*;
use crate::agent::state::agent_dir;

fn entry<'a>(action: &'a str, app: &'a str) -> AuditEntry<'a> {
    AuditEntry {
        agent: "cursor",
        action,
        app,
        success: true,
    }
}

#[test]
fn hostile_action_and_app_stay_one_audit_entry() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    let action = "deploy\r\n2026-01-01 [AI] root destroy prod success";
    let app = "web\nforged";
    log_agent_action(&dir, None, &entry(action, app));
    let log = std::fs::read_to_string(dir.join(AUDIT_FILE)).unwrap();
    assert_eq!(log.lines().count(), 1, "{log:?}");
    let parsed: serde_json::Value = serde_json::from_str(log.trim_end()).unwrap();
    assert_eq!(parsed["action"], action);
    assert_eq!(parsed["app"], app);
    assert_eq!(parsed["agent"], "cursor");
    assert_eq!(parsed["status"], "success");
}

#[test]
fn mutating_command_is_refused_when_the_log_cannot_be_opened() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    let victim = root.path().join("victim");
    std::fs::write(&victim, "keep").unwrap();
    std::os::unix::fs::symlink(&victim, dir.join(AUDIT_FILE)).unwrap();
    assert!(open_before_mutation(&dir, "deploy").is_err());
    assert!(open_before_mutation(&dir, "logs").unwrap().is_none());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
}

use super::*;
use crate::remote::types::ServerName;
use tempfile::TempDir;

fn server() -> Server {
    Server {
        name: ServerName::try_from("edge").unwrap(),
        address: SshAddress::try_from("riku@edge.example").unwrap(),
        admin: None,
    }
}

fn repo(name: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(name);
    std::fs::create_dir(&dir).unwrap();
    let init = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["init", "-q"])
        .status()
        .unwrap();
    assert!(init.success(), "git init failed in {}", dir.display());
    (tmp, dir)
}

fn git_url(dir: &Path, remote: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", remote])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn remote_added_in_a_repo() {
    let (_tmp, dir) = repo("myapp");
    let outcome = add_git_remote(&dir, &server()).unwrap();
    assert_eq!(
        outcome,
        RemoteOutcome::Added("riku@edge.example:myapp".to_string())
    );
    assert_eq!(git_url(&dir, "riku"), "riku@edge.example:myapp");
}

#[test]
fn existing_riku_remote_is_kept() {
    let (_tmp, dir) = repo("myapp");
    let add = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["remote", "add", "riku", "deploy@other.example:shop"])
        .status()
        .unwrap();
    assert!(add.success());

    let outcome = add_git_remote(&dir, &server()).unwrap();
    assert_eq!(outcome, RemoteOutcome::Kept("riku".to_string()));
    assert_eq!(git_url(&dir, "riku"), "deploy@other.example:shop");
}

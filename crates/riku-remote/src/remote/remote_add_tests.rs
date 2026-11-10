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

// The test's own git calls ignore the developer's global and system config
// (templates, hooks, url rewrites), so setup and checks are the same on
// every machine.
fn test_git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn repo(name: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(name);
    std::fs::create_dir(&dir).unwrap();
    let init = test_git(&dir, &["init", "-q", "--template="]);
    assert!(
        init.status.success(),
        "git init failed in {}",
        dir.display()
    );
    (tmp, dir)
}

fn stored_url(dir: &Path, remote: &str) -> String {
    let key = format!("remote.{remote}.url");
    let out = test_git(dir, &["config", "--local", "--get", &key]);
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
    assert_eq!(stored_url(&dir, "riku"), "riku@edge.example:myapp");
}

#[test]
fn existing_riku_remote_is_kept() {
    let (_tmp, dir) = repo("myapp");
    let add = test_git(
        &dir,
        &["remote", "add", "riku", "deploy@other.example:shop"],
    );
    assert!(add.status.success());

    let outcome = add_git_remote(&dir, &server()).unwrap();
    assert_eq!(outcome, RemoteOutcome::Kept("riku".to_string()));
    assert_eq!(stored_url(&dir, "riku"), "deploy@other.example:shop");
}

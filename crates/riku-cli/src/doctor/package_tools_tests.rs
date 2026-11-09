use super::*;
use crate::doctor::Status;

#[test]
fn missing_tool_warns_with_install_command() {
    let check = tool_check(&TOOLS[1], false);

    assert_eq!(check.status, Status::Warn);
    assert!(
        check.detail.contains("pip") && check.detail.contains("astral.sh/uv/install.sh"),
        "{}",
        check.detail
    );
}

#[test]
fn uv_in_home_local_bin_counts_as_installed() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("riku-test-tool-xyz"), "").unwrap();

    assert!(installed("riku-test-tool-xyz", Some(home.path())));
    assert!(!installed("riku-test-tool-xyz", None));
}

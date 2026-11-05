use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use anyhow::anyhow;

use super::*;

fn mock_fetch(url: &str) -> Result<String> {
    if url.ends_with("ips-v4") {
        Ok("192.0.2.0/24\n".to_string())
    } else {
        Ok("2001:db8::/32\n".to_string())
    }
}

#[test]
fn writes_the_ranges_world_readable_into_a_checked_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let owner = tmp.path().metadata().unwrap().uid();
    let file = tmp.path().join("etc/riku/cloudflare.conf");

    write_ranges(&file, owner, mock_fetch).unwrap();

    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("allow 192.0.2.0/24;\nallow 2001:db8::/32;"),
        "{text}"
    );
    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o644);
}

#[test]
fn a_failed_fetch_keeps_the_old_file() {
    let tmp = tempfile::tempdir().unwrap();
    let owner = tmp.path().metadata().unwrap().uid();
    let file = tmp.path().join("cloudflare.conf");
    fs::write(&file, "allow 198.51.100.0/24;\n").unwrap();

    let err = write_ranges(&file, owner, |_| Err(anyhow!("offline"))).unwrap_err();

    assert!(err.to_string().contains("offline"), "{err}");
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "allow 198.51.100.0/24;\n"
    );
}

#[test]
fn refuses_a_directory_another_user_could_write() {
    let tmp = tempfile::tempdir().unwrap();
    let owner = tmp.path().metadata().unwrap().uid();
    let dir = tmp.path().join("etc");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();

    let err = write_ranges(&dir.join("cloudflare.conf"), owner, mock_fetch).unwrap_err();

    assert!(err.to_string().contains("writable"), "{err}");
}

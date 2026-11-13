use std::sync::{Arc, Barrier};

use super::*;

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_private_write_replaces_a_readable_file_with_an_owner_only_one() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_file = tmp.path().join("ENV");
    fs::write(&env_file, "OLD=1\n").unwrap();
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o644)).unwrap();

    write_private(&env_file, b"SECRET=1\n").unwrap();

    assert_eq!(fs::read_to_string(&env_file).unwrap(), "SECRET=1\n");
    assert_eq!(
        mode(&env_file),
        0o600,
        "ENV file is mode {:o}",
        mode(&env_file)
    );
}

#[test]
fn a_symlink_planted_at_the_temp_path_is_not_followed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_file = tmp.path().join("ENV");
    let victim = tmp.path().join("victim");
    fs::write(&victim, "untouched").unwrap();
    let planted = temp_path(&env_file, 0);
    std::os::unix::fs::symlink(&victim, &planted).unwrap();
    let mut seq = 0..;

    write_atomic_with(&env_file, b"SECRET=1\n", Access::Private, || {
        seq.next().unwrap()
    })
    .unwrap();

    assert_eq!(fs::read_to_string(&victim).unwrap(), "untouched");
    assert_eq!(fs::read_to_string(&env_file).unwrap(), "SECRET=1\n");
    assert!(
        fs::symlink_metadata(&planted)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the planted link was replaced instead of skipped"
    );
}

#[test]
fn two_threads_writing_one_file_both_succeed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("same");
    for round in 0..50 {
        let barrier = Arc::new(Barrier::new(2));
        let writers: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|name| {
                let path = path.clone();
                let barrier = barrier.clone();
                let text = format!("{name}{round}");
                std::thread::spawn(move || {
                    barrier.wait();
                    write_private(&path, text.as_bytes())
                })
            })
            .collect();
        for writer in writers {
            let outcome = writer.join().unwrap();
            assert!(outcome.is_ok(), "round {round}: {outcome:?}");
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text == format!("a{round}") || text == format!("b{round}"),
            "round {round}: file holds {text:?}"
        );
    }
}

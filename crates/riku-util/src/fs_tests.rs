use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn a_private_dir_is_tightened_when_it_already_exists() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_dir = tmp.path().join("envs").join("shop");
    fs::create_dir_all(&env_dir).unwrap();
    fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o755)).unwrap();

    create_private_dir(&env_dir).unwrap();

    let mode = fs::metadata(&env_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "env dir is mode {mode:o}");
}

/// Two threads of one process swap the same link. With a temp name shared
/// by the process, one removed or renamed the other's temp link and its
/// rename failed.
#[test]
fn two_threads_swapping_one_link_never_fail_or_break_it() {
    let tmp = tempfile::TempDir::new().unwrap();
    let targets = [tmp.path().join("a.toml"), tmp.path().join("b.toml")];
    for target in &targets {
        fs::write(target, "").unwrap();
    }
    let link = tmp.path().join("enabled.toml");
    replace_symlink(&targets[0], &link).unwrap();
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));

    let swappers: Vec<_> = targets
        .iter()
        .cloned()
        .map(|target| {
            let (link, start) = (link.clone(), std::sync::Arc::clone(&start));
            std::thread::spawn(move || {
                start.wait();
                (0..50)
                    .map(|_| replace_symlink(&target, &link).map_err(|e| format!("{e:#}")))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let outcomes: Vec<_> = swappers
        .into_iter()
        .flat_map(|swapper| swapper.join().unwrap())
        .collect();

    let failures: Vec<_> = outcomes.iter().filter_map(|o| o.as_ref().err()).collect();
    assert!(failures.is_empty(), "swaps failed: {failures:?}");
    let now = fs::read_link(&link).expect("the link must still exist");
    assert!(targets.contains(&now), "the link points at {now:?}");
    let leftovers: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().starts_with('.'))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp links left behind: {leftovers:?}"
    );
}

use super::*;

fn paths() -> (tempfile::TempDir, RikuPaths) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    (tmp, paths)
}

fn port_entry(port: u16) -> Entry {
    Entry {
        mode: RoutingMode::Port,
        port: Some(port),
        picked: true,
        tls: None,
        cert_for: None,
        domains: Vec::new(),
        fallback: false,
    }
}

#[test]
fn missing_state_file_is_empty() {
    let (_tmp, paths) = paths();

    let state = RoutingState::load(&paths).unwrap();

    assert_eq!(state, RoutingState::default());
}

#[test]
fn state_round_trips_through_the_lock() {
    let (_tmp, paths) = paths();

    with_routing_lock(&paths, |state| {
        state.set_entry("myapp", port_entry(8100));
        Ok(())
    })
    .unwrap();
    let state = RoutingState::load(&paths).unwrap();

    assert_eq!(state.entry("myapp"), Some(&port_entry(8100)));
}

#[test]
fn a_failed_closure_writes_nothing() {
    let (_tmp, paths) = paths();

    let outcome: Result<()> = with_routing_lock(&paths, |state| {
        state.set_entry("myapp", port_entry(8100));
        Err(anyhow::anyhow!("refused"))
    });

    assert!(outcome.is_err());
    assert_eq!(RoutingState::load(&paths).unwrap().entry("myapp"), None);
}

#[test]
fn forget_drops_the_entry() {
    let (_tmp, paths) = paths();
    with_routing_lock(&paths, |state| {
        state.set_entry("myapp", port_entry(8100));
        Ok(())
    })
    .unwrap();

    forget(&paths, "myapp");

    assert_eq!(RoutingState::load(&paths).unwrap().entry("myapp"), None);
}

#[test]
fn restore_puts_back_the_entry_from_before_a_failed_apply() {
    let (_tmp, paths) = paths();
    let before = Entry {
        mode: RoutingMode::Path,
        port: None,
        picked: false,
        tls: None,
        cert_for: None,
        domains: Vec::new(),
        fallback: false,
    };
    with_routing_lock(&paths, |state| {
        state.set_entry("myapp", port_entry(8100));
        state.set_entry("newapp", port_entry(8101));
        Ok(())
    })
    .unwrap();

    restore(&paths, "myapp", Some(before.clone()));
    restore(&paths, "newapp", None);
    let state = RoutingState::load(&paths).unwrap();

    assert_eq!(state.entry("myapp"), Some(&before));
    assert_eq!(
        state.entry("newapp"),
        None,
        "a first apply that failed leaves no entry"
    );
}

#[test]
fn unknown_field_in_state_is_refused() {
    let (_tmp, paths) = paths();
    std::fs::create_dir_all(state_dir(&paths)).unwrap();
    std::fs::write(
        state_dir(&paths).join(STATE_FILE),
        "[apps.myapp]\nmode = \"port\"\nshell = \"x\"\n",
    )
    .unwrap();

    assert!(RoutingState::load(&paths).is_err());
}

use super::*;

fn configured(home: &Path) -> Option<String> {
    read_default_branch(home).unwrap()
}

#[test]
fn writes_main_when_nothing_is_configured() {
    let home = tempfile::tempdir().unwrap();

    set_default_branch(home.path(), None).unwrap();

    assert_eq!(configured(home.path()).as_deref(), Some("main"));
}

#[test]
fn keeps_an_existing_branch_unless_the_env_names_one() {
    let home = tempfile::tempdir().unwrap();
    write_default_branch(home.path(), "trunk").unwrap();

    set_default_branch(home.path(), None).unwrap();
    assert_eq!(configured(home.path()).as_deref(), Some("trunk"));

    set_default_branch(home.path(), Some("production".to_string())).unwrap();
    assert_eq!(configured(home.path()).as_deref(), Some("production"));
}

#[test]
fn rejects_a_name_git_would_refuse() {
    let home = tempfile::tempdir().unwrap();

    let err = set_default_branch(home.path(), Some("bad..name".to_string())).unwrap_err();

    assert!(err.to_string().contains("bad..name"), "{err}");
    assert_eq!(configured(home.path()), None);
}

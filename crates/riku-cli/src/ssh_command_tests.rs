use super::*;

#[test]
fn keeps_quoted_argument_whole() {
    let argv = split_original(r#"run myapp echo "a b""#).unwrap();
    assert_eq!(argv, ["run", "myapp", "echo", "a b"]);
}

#[test]
fn drops_leading_riku_word() {
    assert_eq!(split_original("riku ps myapp").unwrap(), ["ps", "myapp"]);
}

#[test]
fn git_push_command_loses_its_quotes() {
    assert_eq!(
        split_original("git-receive-pack 'myapp'").unwrap(),
        ["git-receive-pack", "myapp"]
    );
}

#[test]
fn does_not_expand_globs_or_variables() {
    assert_eq!(
        split_original("run app ls * $HOME").unwrap(),
        ["run", "app", "ls", "*", "$HOME"]
    );
}

#[test]
fn rejects_unbalanced_quote() {
    let err = split_original(r#"run app echo "a"#).unwrap_err();
    assert!(err.to_string().contains("unbalanced"), "got: {err}");
}

#[test]
fn no_subcommand_is_named_riku() {
    use clap::CommandFactory;
    let names: Vec<String> = crate::cli::Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    assert!(
        !names.iter().any(|n| n == "riku"),
        "dropping a leading riku word would shadow it"
    );
}

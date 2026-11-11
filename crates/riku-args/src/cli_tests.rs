use super::*;

#[test]
fn cli_version_is_the_crate_version() {
    use clap::CommandFactory;

    assert_eq!(
        Cli::command().get_version(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn agent_accepts_the_scope_of_a_forced_command() {
    let cli = Cli::try_parse_from(["riku", "agent", "--scope", "staging", "apps"])
        .expect("`riku agent --scope staging` must parse");
    match cli.command {
        Commands::Agent { scope, command, .. } => {
            assert_eq!(scope.as_deref(), Some("staging"));
            assert_eq!(command.as_deref(), Some("apps"));
        }
        other => panic!("expected the agent command, got {other:?}"),
    }
}

fn agent_args(argv: &[&str]) -> (Vec<String>, Option<String>) {
    match Cli::try_parse_from(argv) {
        Ok(Cli {
            command: Commands::Agent { args, confirm, .. },
            ..
        }) => (args, confirm),
        other => panic!("expected the agent command from {argv:?}, got {other:?}"),
    }
}

#[test]
fn agent_command_arguments_need_no_separator() {
    let plain = agent_args(&["riku", "agent", "destroy", "myapp", "--confirm", "tok"]);
    assert_eq!(plain, (vec!["myapp".to_string()], Some("tok".to_string())));

    let separated = agent_args(&["riku", "agent", "run", "myapp", "--", "ls", "-la"]);
    assert_eq!(separated.0, ["myapp", "ls", "-la"]);
}

fn destroy_force(argv: &[&str]) -> bool {
    match Cli::try_parse_from(argv) {
        Ok(Cli {
            command: Commands::Destroy { force, .. },
            ..
        }) => force,
        other => panic!("expected the destroy command from {argv:?}, got {other:?}"),
    }
}

#[test]
fn destroy_force_is_off_unless_asked() {
    assert!(destroy_force(&["riku", "destroy", "myapp", "--force"]));
    assert!(!destroy_force(&["riku", "destroy", "myapp"]));
}

#[test]
fn restart_refuses_hot_with_named_service() {
    let err = match Cli::try_parse_from(["riku", "restart", "myapp", "web", "--hot"]) {
        Ok(cli) => panic!("expected parse to fail, got {cli:?}"),
        Err(err) => err,
    };
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::ArgumentConflict,
        "expected ArgumentConflict, got {:?}",
        err.kind()
    );
}

#[test]
fn restart_accepts_hot_without_service() {
    let cli = match Cli::try_parse_from(["riku", "restart", "myapp", "--hot"]) {
        Ok(cli) => cli,
        Err(err) => panic!("failed to parse restart with --hot: {err}"),
    };
    match cli.command {
        Commands::Restart { app, service, hot } => {
            assert_eq!(app.as_deref(), Some("myapp"));
            assert_eq!(service, None);
            assert!(hot, "expected hot to be true");
        }
        other => panic!("expected restart command, got {other:?}"),
    }
}

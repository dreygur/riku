use super::*;

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

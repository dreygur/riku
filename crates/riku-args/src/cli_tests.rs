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

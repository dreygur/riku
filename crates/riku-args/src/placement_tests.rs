use super::*;
use clap::CommandFactory;

#[test]
fn every_subcommand_has_a_placement() {
    let missing: Vec<String> = crate::cli::Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .filter(|n| n != "help" && placement_of(n).is_none())
        .collect();
    assert!(missing.is_empty(), "no placement for: {missing:?}");
}

#[test]
fn rk_runs_local_forwards_and_refuses_the_rest() {
    assert_eq!(client_rule("server"), ClientRule::RunsHere);
    assert_eq!(client_rule("dashboard"), ClientRule::RunsHere);
    assert_eq!(client_rule("apps"), ClientRule::Forwards);
    assert_eq!(
        client_rule("container"),
        ClientRule::Refused(Elsewhere::FullBinary)
    );
    assert_eq!(
        client_rule("init"),
        ClientRule::Refused(Elsewhere::OnServer)
    );
}

#[test]
fn unknown_name_has_no_placement() {
    assert_eq!(placement_of("frobnicate"), None);
}

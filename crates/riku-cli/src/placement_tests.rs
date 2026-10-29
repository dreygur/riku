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
fn unknown_name_has_no_placement() {
    assert_eq!(placement_of("frobnicate"), None);
}

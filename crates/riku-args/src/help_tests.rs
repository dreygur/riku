use super::*;
use clap::CommandFactory;

#[test]
fn help_lists_groups_in_order_and_each_visible_command_once() {
    let text = command("Forwarded commands:").render_help().to_string();
    let (l, s, f) = (
        text.find("Local commands:").unwrap(),
        text.find("Server commands:").unwrap(),
        text.find("Forwarded commands").unwrap(),
    );
    assert!(l < s && s < f, "group order wrong:\n{text}");
    let root = Cli::command();
    for sub in root.get_subcommands().filter(|c| !c.is_hide_set()) {
        let name = sub.get_name();
        assert_eq!(
            text.matches(&format!("\n  {name} ")).count(),
            1,
            "{name} not listed exactly once:\n{text}"
        );
    }
    assert!(!text.contains("git-hook"), "hidden command shown:\n{text}");
}

#[test]
fn client_help_leaves_out_what_a_laptop_cannot_run() {
    let text = client_command(None).render_help().to_string();
    for shown in ["\n  server ", "\n  dashboard ", "\n  apps "] {
        assert!(text.contains(shown), "{shown:?} missing:\n{text}");
    }
    for hidden in ["\n  init ", "\n  container ", "Server commands:", "--local"] {
        assert!(!text.contains(hidden), "{hidden:?} shown:\n{text}");
    }
}

#[test]
fn braces_in_descriptions_do_not_break_the_template() {
    assert_eq!(escape_braces("a {b} c"), "a (b) c");
}

#[test]
fn heading_names_the_implied_server() {
    let name = ServerName::try_from("edge").unwrap();
    assert_eq!(
        forward_heading(Some(&name)),
        "Forwarded commands (run on edge, or here with --local):"
    );
    assert_eq!(forward_heading(None), FORWARD_HEADING);
}

#[test]
fn clap_definitions_have_no_flag_clashes() {
    Cli::command().debug_assert();
}

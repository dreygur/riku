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
fn braces_in_descriptions_do_not_break_the_template() {
    assert_eq!(escape_braces("a {b} c"), "a (b) c");
}

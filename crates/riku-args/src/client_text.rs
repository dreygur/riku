//! Help text of the shared command tree that reads wrong for `rk`.
//!
//! The clap definitions are written for `riku`. `rk` shows the same tree, so
//! the examples are rewritten to its own name and the two commands that mean
//! something else on a laptop get their own description.

use clap::Command;

/// Examples in the definitions are indented two spaces and start with `riku`.
const RIKU_EXAMPLE: &str = "  riku ";

const ABOUT_FOR_CLIENT: [(&str, &str); 2] = [
    (
        "dashboard",
        "Open the selected server's dashboard in your browser",
    ),
    ("update", "Update the riku binary on the selected server"),
];

fn rename_examples(cmd: Command, program: &str) -> Command {
    let renamed = cmd.get_after_help().map(|text| {
        text.to_string()
            .replace(RIKU_EXAMPLE, &format!("  {program} "))
    });
    let cmd = match renamed {
        Some(text) => cmd.after_help(text),
        None => cmd,
    };
    cmd.mut_subcommands(|sub| rename_examples(sub, program))
}

/// `root` with its examples and descriptions made right for `program`.
pub(crate) fn for_client(root: Command, program: &str) -> Command {
    let described = ABOUT_FOR_CLIENT
        .into_iter()
        .fold(root, |cmd, (name, about)| {
            cmd.mut_subcommand(name, |sub| sub.about(about))
        });
    rename_examples(described, program)
}

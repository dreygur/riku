use super::*;

fn plain(mark: Mark, label: &str, detail: &str) -> String {
    render_line(mark, label, detail, false)
}

#[test]
fn plain_mode_renders_the_rail_without_escapes() {
    let lines = [
        plain(Mark::Intro, "riku init", "v0.1.4"),
        plain(Mark::Rail, "", ""),
        plain(Mark::Step, "Router", "caddy"),
        plain(Mark::Warn, "Fallback domain off", ""),
        plain(Mark::Rail, "192.168.10.121 is private", ""),
        plain(Mark::Error, "Build failed", "exit code 1"),
        plain(Mark::Outro, "Riku is ready", ""),
    ];
    let summary = render_summary(
        &[("Router", "caddy"), ("Next", "git push riku main")],
        false,
    );
    let text = format!("{}\n{}", lines.join("\n"), summary);
    assert_eq!(
        text,
        "┌  riku init  v0.1.4\n\
         │\n\
         ◇  Router  caddy\n\
         ▲  Fallback domain off\n\
         │  192.168.10.121 is private\n\
         ■  Build failed  exit code 1\n\
         └  Riku is ready\n\
         \n   Router   caddy\
         \n   Next     git push riku main"
    );
    assert!(
        !text.contains('\x1b'),
        "plain mode must not emit ANSI escapes"
    );
}

#[test]
fn a_multi_line_message_continues_on_the_rail() {
    assert_eq!(
        plain(
            Mark::Error,
            "build ran out of memory\n\nraise RIKU_MAX_MEMORY_MB",
            ""
        ),
        "■  build ran out of memory\n│\n│  raise RIKU_MAX_MEMORY_MB"
    );
}

#[test]
fn colour_mode_paints_the_glyph() {
    let line = render_line(Mark::Step, "Router", "caddy", true);
    assert!(line.starts_with("\x1b[32m◇\x1b[0m  Router"), "got {line:?}");
}

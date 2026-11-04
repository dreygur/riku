use std::collections::HashMap;
use std::fs;

use super::*;

fn env_naming(file: &str) -> HashMap<String, String> {
    HashMap::from([(SNIPPET_KEY.to_string(), file.to_string())])
}

fn snippet_result(content: &str) -> Result<String> {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("nginx.include"), content).unwrap();
    read_app_snippet(&env_naming("nginx.include"), tmp.path())
}

#[test]
fn allowed_directives_comments_and_a_quoted_hash_are_copied() {
    let content = "# limits\nclient_max_body_size 10m; # inline\nadd_header X \"a#b\";\nadd_header X-XSS-Protection \"1; mode=block\" always;\ngzip on;\n";

    assert_eq!(snippet_result(content).unwrap(), content);
}

#[test]
fn bypasses_open_quotes_and_unlisted_directives_are_refused_by_reason() {
    let refused = [
        (
            "add_header X 1; } server { server_name other.example; }",
            "{ or }",
        ),
        (
            "add_header X \"a#\"; } server { server_name other.example; ...",
            "{ or }",
        ),
        (
            "add_header X \"\n#\"; access_log /etc/cron.d/x; set $a \"\n\";",
            "runs past the end of its line",
        ),
        ("add_header X \"never closed;", "never closed"),
        ("add_header X a#;include x;", "inside a word"),
        ("add_header X a\"; include x; #\";", "inside a word"),
        ("add_header X \"a\"b;", "followed by a space"),
        ("add_header X a\\ #; include x;", "a backslash"),
        ("add_header X a\\\n#; include x;", "a backslash"),
        ("add_header X caf\u{e9};", "printable ASCII"),
        ("add_header X a\r\ninclude x;", "printable ASCII"),
        ("proxy_pass http://127.0.0.1:9;", "`proxy_pass`"),
        ("client_max_body_size 1m; include /etc/shadow;", "`include`"),
        ("# a comment\naccess_log /tmp/x;", "`access_log`"),
        ("\"add_header\" X 1;", "`\"add_header\"`"),
        ("gzip on;\ngzip_vary on", "no closing ;"),
        ("gzip on;;", "empty statement"),
        ("; gzip on;", "empty statement"),
    ];
    for (content, reason) in refused {
        match snippet_result(content) {
            Ok(_) => panic!("accepted snippet {content:?}"),
            Err(e) => assert!(e.to_string().contains(reason), "{content:?}: {e}"),
        }
    }
}

/// Installs a real filter plugin that answers every `nginx.include_content`
/// call with `reply` as the new snippet.
fn install_rewriting_filter(paths: &crate::config::RikuPaths, reply: &str) {
    use std::os::unix::fs::PermissionsExt;

    let bundle = paths.plugin_root.join("rewriter");
    fs::create_dir_all(bundle.join("bin")).unwrap();
    let script = bundle.join("bin/on-filter");
    fs::write(
        &script,
        format!("#!/bin/sh\nread line\nprintf '%s\\n' '{{\"data\":\"{reply}\"}}'\n"),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        bundle.join("riku-plugin.toml"),
        format!(
            "name=\"rewriter\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-filter\"\n[filters]\nsubscribe=[\"{INCLUDE_FILTER}\"]\n",
            riku_plugins::RIKU_PLUGIN_API
        ),
    )
    .unwrap();
}

#[test]
fn a_filter_rewriting_the_snippet_into_a_refused_directive_fails_the_deploy() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    fs::create_dir(&app).unwrap();
    fs::write(app.join("nginx.include"), "gzip on;\n").unwrap();
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    install_rewriting_filter(&paths, "gzip on; include /etc/shadow;");

    let mut context = tera::Context::new();
    let err = insert_include_file(&mut context, &env_naming("nginx.include"), &app, &paths)
        .expect_err("a filter adding `include` must refuse the deploy")
        .to_string();

    assert!(
        err.contains(INCLUDE_FILTER) && err.contains("`include`"),
        "error should name the filter stage and the directive: {err}"
    );
    assert!(
        context.get("RIKU_NGINX_INCLUDE_CONTENT").is_none(),
        "refused content reached the template context"
    );
}

#[test]
fn a_file_outside_the_app_or_missing_is_refused_by_path() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    fs::create_dir(&app).unwrap();
    fs::write(tmp.path().join("secret.conf"), "add_header X 1;").unwrap();
    std::os::unix::fs::symlink(tmp.path().join("secret.conf"), app.join("escape.conf")).unwrap();

    for (name, expected) in [
        ("escape.conf", "outside the app directory"),
        ("../secret.conf", "outside the app directory"),
        ("missing.conf", "does not exist"),
    ] {
        let err = read_app_snippet(&env_naming(name), &app)
            .expect_err(name)
            .to_string();
        assert!(
            err.contains(expected) && err.contains(name),
            "{name}: {err}"
        );
    }
}

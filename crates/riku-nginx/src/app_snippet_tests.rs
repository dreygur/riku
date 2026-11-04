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

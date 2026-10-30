use super::*;
use crate::remote::SshAddress;
use crate::routing::build_plugin_args;

#[test]
fn hint_fills_only_an_omitted_optional_app() {
    let mut omitted = Commands::Stop { app: None };
    assert!(apply_app_hint(&mut omitted, Some("shop")));
    assert!(
        matches!(&omitted, Commands::Stop { app: Some(app) } if app == "shop"),
        "{omitted:?}"
    );

    let mut typed = Commands::Stop {
        app: Some("blog".to_string()),
    };
    assert!(!apply_app_hint(&mut typed, Some("shop")));
    assert!(
        matches!(&typed, Commands::Stop { app: Some(app) } if app == "blog"),
        "{typed:?}"
    );

    let mut required = Commands::Run {
        app: "blog".to_string(),
        cmd: vec!["ls".to_string()],
    };
    assert!(!apply_app_hint(&mut required, Some("shop")));
}

#[test]
fn destroy_requires_a_typed_app() {
    let parsed =
        crate::help::command("Forwarded commands:").try_get_matches_from(["riku", "destroy"]);
    assert!(parsed.is_err(), "riku destroy parsed without an app name");
}

#[test]
fn client_plugin_sees_the_hinted_app() {
    let addr = SshAddress::try_from("riku@edge.example").unwrap();
    let mut cmd = Commands::Restart {
        app: None,
        hot: false,
    };
    apply_app_hint(&mut cmd, Some("shop"));
    assert_eq!(build_plugin_args(&cmd, Some(&addr))[1], "shop");
}

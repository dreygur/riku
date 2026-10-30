use super::*;
use crate::remote::SshAddress;
use riku_args::app_hint::apply_app_hint;

#[test]
fn client_plugin_receives_selected_server() {
    let addr = SshAddress::try_from("riku@edge.example").unwrap();
    let cmd = Commands::Stop {
        app: Some("myapp".to_string()),
    };
    assert_eq!(
        build_plugin_args(&cmd, Some(&addr))[0],
        "riku@edge.example"
    );
    assert_eq!(build_plugin_args(&cmd, None)[0], "");
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

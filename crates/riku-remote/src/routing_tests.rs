use super::*;
use crate::remote::SshAddress;
use riku_args::app_hint::apply_app_hint;

#[test]
fn client_plugin_receives_selected_server() {
    let addr = SshAddress::try_from("riku@edge.example").unwrap();
    let cmd = Commands::Stop {
        app: Some("myapp".to_string()),
        service: None,
    };
    assert_eq!(build_plugin_args(&cmd, Some(&addr))[0], "riku@edge.example");
    assert_eq!(build_plugin_args(&cmd, None)[0], "");
}

#[test]
fn client_plugin_sees_the_hinted_app() {
    let addr = SshAddress::try_from("riku@edge.example").unwrap();
    let mut cmd = Commands::Restart {
        app: None,
        service: None,
        hot: false,
    };
    apply_app_hint(&mut cmd, Some("shop"));
    assert_eq!(build_plugin_args(&cmd, Some(&addr))[1], "shop");
}

#[test]
fn client_plugin_receives_service_for_restart_and_stop() {
    let restart_cmd = Commands::Restart {
        app: Some("shop".to_string()),
        service: Some("web".to_string()),
        hot: false,
    };
    let restart_args = build_plugin_args(&restart_cmd, None);
    assert_eq!(
        &restart_args[2..],
        &["restart", "web"],
        "expected ['restart', 'web'], got {:?}",
        &restart_args[2..]
    );

    let stop_cmd = Commands::Stop {
        app: Some("shop".to_string()),
        service: Some("worker".to_string()),
    };
    let stop_args = build_plugin_args(&stop_cmd, None);
    assert_eq!(
        &stop_args[2..],
        &["stop", "worker"],
        "expected ['stop', 'worker'], got {:?}",
        &stop_args[2..]
    );
}

#[test]
fn client_plugin_receives_hot_flag_for_restart() {
    let cmd = Commands::Restart {
        app: Some("shop".to_string()),
        service: None,
        hot: true,
    };
    let args = build_plugin_args(&cmd, None);
    assert_eq!(
        &args[2..],
        &["restart", "--hot"],
        "expected ['restart', '--hot'], got {:?}",
        &args[2..]
    );
}

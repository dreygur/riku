use super::*;
use crate::remote::SshAddress;

#[test]
fn client_plugin_receives_selected_server() {
    let addr = SshAddress::try_from("riku@solutech.local").unwrap();
    let cmd = Commands::Stop {
        app: Some("myapp".to_string()),
    };
    assert_eq!(
        build_plugin_args(&cmd, Some(&addr))[0],
        "riku@solutech.local"
    );
    assert_eq!(build_plugin_args(&cmd, None)[0], "");
}

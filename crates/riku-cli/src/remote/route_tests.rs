use super::*;

#[test]
fn dashboard_is_the_only_server_command_that_resolves_a_server() {
    assert_eq!(routing("dashboard", true), Routing::Resolve);
    assert_eq!(routing("dashboard", false), Routing::Resolve);
    assert_eq!(routing("init", true), Routing::Refuse);
    assert_eq!(routing("init", false), Routing::Local);
}

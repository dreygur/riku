use super::*;

#[test]
fn plugin_router_orders_after_the_network_only() {
    assert_eq!(
        unit_ordering(&Router::Plugin("traefik".to_string())),
        "After=network.target"
    );
}

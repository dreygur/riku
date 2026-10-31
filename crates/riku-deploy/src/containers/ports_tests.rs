use std::net::TcpListener;

use super::*;

#[test]
fn free_port_can_be_bound_right_after() {
    let port = pick_free_port().unwrap();

    assert_ne!(port, 0);
    TcpListener::bind(("127.0.0.1", port)).expect("picked port was not free");
}

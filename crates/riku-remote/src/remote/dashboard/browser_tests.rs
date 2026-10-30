use std::net::TcpListener;

use super::*;

#[test]
fn falls_back_to_a_free_port_when_the_preferred_one_is_taken() {
    let taken = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let busy = taken.local_addr().unwrap().port();
    let port = pick_local_port(busy).unwrap();
    assert_ne!(port, busy, "picked the port another listener holds");
    assert_ne!(port, 0);
}

#[test]
fn url_points_at_the_local_forward_with_the_token() {
    assert_eq!(
        dashboard_url(8088, "abc"),
        "http://127.0.0.1:8088/?token=abc"
    );
}

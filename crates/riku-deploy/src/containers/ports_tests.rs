use std::io::ErrorKind;
use std::net::TcpListener;

use super::*;

// The picked port is released before pick_free_port returns, so re-binding it
// would race with any other test binding port 0. The test holds the listener
// instead, which proves the port is real and exclusive without a race.
#[test]
fn picked_port_is_a_real_loopback_port_while_held() {
    let (_listener, port) = hold_free_port().unwrap();

    assert_ne!(port, 0);
    let second = TcpListener::bind(("127.0.0.1", port)).expect_err("port was bound twice");
    assert_eq!(second.kind(), ErrorKind::AddrInUse);
    assert_ne!(pick_free_port().unwrap(), 0);
}

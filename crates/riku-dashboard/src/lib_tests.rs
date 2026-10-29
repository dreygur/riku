use super::*;

#[tokio::test]
async fn reports_real_port_when_binding_zero() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (listener, line) = bind_reporting(addr).await.unwrap();
    let port: u16 = line
        .strip_prefix("riku-dashboard-bound ")
        .and_then(|rest| rest.parse().ok())
        .unwrap_or_else(|| panic!("unexpected report line {line:?}"));
    assert_ne!(port, 0);
    assert_eq!(port, listener.local_addr().unwrap().port());
}

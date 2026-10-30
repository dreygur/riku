use super::*;

#[test]
fn backend_renders_router_address() {
    assert_eq!(Backend::loopback(5000).to_string(), "127.0.0.1:5000");
    assert_eq!(
        Backend::Tcp {
            host: "::1".parse().unwrap(),
            port: 5000
        }
        .to_string(),
        "[::1]:5000"
    );
    assert_eq!(
        Backend::Unix("/srv/riku/nginx/myapp.sock".into()).to_string(),
        "unix:/srv/riku/nginx/myapp.sock"
    );
}

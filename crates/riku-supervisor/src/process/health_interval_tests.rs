use super::super::test_support::minimal_config;
use super::ProcessManager;
use crate::config::HealthCheck;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

fn counting_health_endpoint() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        }
    });
    (port, hits)
}

#[test]
fn health_check_honours_interval() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("test.log");
    let (port, hits) = counting_health_endpoint();
    let health = HealthCheck {
        url: "/health".to_string(),
        interval: 30,
        timeout: 5,
        retries: 3,
    };
    let mut config = minimal_config(
        "sleep 30",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );
    config.env.insert("PORT".to_string(), port.to_string());
    config.options.health_check = Some(health.clone());

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config).expect("spawn should succeed");

    for _ in 0..2 {
        pm.run_health_checks(
            vec![("testapp@web.1".to_string(), health.clone())],
            &mut vec![],
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "two ticks inside a 30 s interval must run exactly one probe"
    );
}

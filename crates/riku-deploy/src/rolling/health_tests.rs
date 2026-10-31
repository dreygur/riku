use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use super::*;
use crate::mock_engine::{mock_engine, project_in};

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn container_id() -> ContainerId {
    ContainerId::try_from(ID).unwrap()
}

fn service_with(extra: &str) -> ServiceDef {
    let dir = tempfile::tempdir().unwrap();
    let compose = format!("services:\n  web:\n    image: nginx:1\n{extra}");
    let (_, project) = project_in(dir.path(), "myapp", &compose);
    project.services[0].clone()
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn answer_with(listener: TcpListener, statuses: &'static [&'static str], served: Arc<AtomicUsize>) {
    thread::spawn(move || {
        for status in statuses {
            let (mut stream, _) = listener.accept().unwrap();
            served.fetch_add(1, Ordering::SeqCst);
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let reply =
                format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
}

#[test]
fn url_needs_2xx() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let served = Arc::new(AtomicUsize::new(0));
    answer_with(
        listener,
        &["503 Service Unavailable", "200 OK"],
        Arc::clone(&served),
    );
    let target = HealthTarget::Url {
        port,
        path: "/healthz".to_string(),
    };

    let outcome = wait_healthy(&target, None);

    assert!(outcome.is_ok(), "503 then 200 should pass: {outcome:?}");
    assert_eq!(
        served.load(Ordering::SeqCst),
        2,
        "the 503 must not count as healthy"
    );
}

#[test]
fn exited_container_fails_fast() {
    let dir = tempfile::tempdir().unwrap();
    let inspect = format!(
        r#"[{{"Id":"{ID}","Name":"/myapp-web-1","Image":"sha256:x","Config":{{"Image":"nginx:1","Labels":{{}}}},"State":{{"Status":"exited","ExitCode":3}}}}]"#
    );
    let (engine, _log) = mock_engine(dir.path(), &format!("echo '{inspect}'"));
    let started = Instant::now();

    let err = wait_healthy(&HealthTarget::Engine(container_id()), Some(&engine)).unwrap_err();

    assert!(
        err.to_string().contains("exited with code 3"),
        "unexpected error: {err}"
    );
    assert!(
        started.elapsed() < HEALTHCHECK_WAIT,
        "waited the full limit instead of failing at once"
    );
}

#[test]
fn closed_port_times_out() {
    let err = wait_healthy(&HealthTarget::Port(closed_port()), None).unwrap_err();

    assert!(
        err.to_string().contains("not healthy after 1s"),
        "unexpected error: {err}"
    );
}

#[test]
fn health_label_beats_port() {
    let service = service_with("    labels:\n      riku.health: /healthz\n");

    let target = health_target(&service, &container_id(), Some(8080));

    assert_eq!(
        target,
        Some(HealthTarget::Url {
            port: 8080,
            path: "/healthz".to_string()
        })
    );
}

#[test]
fn disabled_healthcheck_falls_back_to_port() {
    let service = service_with("    healthcheck:\n      disable: true\n");

    let target = health_target(&service, &container_id(), Some(8080));

    assert_eq!(target, Some(HealthTarget::Port(8080)));
}

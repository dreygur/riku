use super::*;

#[test]
fn worker_user_is_read_from_the_main_user_directive() {
    let conf = "# user nobody;\nuser_agent off;\nuser nginx nginx;\nworker_processes auto;\n";
    assert_eq!(configured_user(conf).as_deref(), Some("nginx"));
    assert_eq!(configured_user("worker_processes 1;\n"), None);
}

#[test]
fn a_worker_that_would_spread_the_sockets_is_refused() {
    assert!(check_worker("www-data", 33, 33).is_ok());
    for worker in [
        ("root", 0, 0),
        ("nobody", 65534, 65534),
        ("nginx", 0, 990),
        ("nginx", 990, 0),
        ("-o", 990, 990),
    ] {
        let (name, uid, gid) = worker;
        assert!(
            check_worker(name, uid, gid).is_err(),
            "worker (name, uid, gid) {worker:?} was accepted"
        );
    }
}

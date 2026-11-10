use super::*;

const BOTH: [Router; 2] = [Router::Nginx, Router::Caddy];

#[test]
fn requested_router_wins() {
    let requested = Some(Router::Plugin("traefik".into()));

    assert_eq!(
        choose_router(&BOTH, requested, Some(Router::Nginx), false, false),
        RouterChoice::Use(Router::Plugin("traefik".into()))
    );
}

#[test]
fn stored_router_wins_over_detection() {
    assert_eq!(
        choose_router(&[Router::Nginx], None, Some(Router::Caddy), false, false),
        RouterChoice::Use(Router::Caddy)
    );
}

#[test]
fn existing_tree_keeps_nginx_without_a_stored_choice() {
    assert_eq!(
        choose_router(&BOTH, None, None, false, false),
        RouterChoice::Use(Router::Nginx)
    );
}

#[test]
fn single_installed_router_is_used_on_a_fresh_install() {
    assert_eq!(
        choose_router(&[Router::Caddy], None, None, true, false),
        RouterChoice::Use(Router::Caddy)
    );
}

#[test]
fn two_routers_ask_when_interactive() {
    assert_eq!(
        choose_router(&BOTH, None, None, true, true),
        RouterChoice::Ask(BOTH.to_vec())
    );
}

#[test]
fn two_routers_without_a_terminal_name_riku_router() {
    let RouterChoice::Fail(message) = choose_router(&BOTH, None, None, true, false) else {
        panic!("expected a failure");
    };
    assert!(
        message.contains("RIKU_ROUTER=nginx") && message.contains("RIKU_ROUTER=caddy"),
        "{message}"
    );
}

#[test]
fn no_router_on_a_fresh_install_fails_with_the_install_command() {
    let RouterChoice::Fail(message) = choose_router(&[], None, None, true, false) else {
        panic!("expected a failure");
    };
    assert!(message.contains("apt install nginx"), "{message}");
}

#[test]
fn an_answer_names_one_of_the_offered_routers() {
    assert_eq!(router_from_answer("caddy\n", &BOTH), Some(Router::Caddy));
    assert_eq!(router_from_answer("  nginx ", &BOTH), Some(Router::Nginx));
    assert_eq!(router_from_answer("apache", &BOTH), None);
    assert_eq!(router_from_answer("", &BOTH), None);
}

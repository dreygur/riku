use super::*;

#[test]
fn the_prerequisites_step_names_git_and_every_router_found() {
    assert_eq!(
        found_tools(true, &[Router::Nginx, Router::Caddy]),
        "git, nginx, caddy"
    );
    assert_eq!(
        found_tools(true, &[]),
        "git, no router",
        "a host with no router must say so, not look complete"
    );
}

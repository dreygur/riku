use super::*;

#[test]
fn tmpfiles_recreates_a_setgid_directory_only_the_socket_group_can_enter() {
    assert_eq!(
        tmpfiles_line("riku"),
        "d /run/riku-sockets 2750 riku riku-sockets -\n"
    );
}

#[test]
fn account_names_that_could_be_misread_are_refused() {
    assert!(is_account_name("www-data"));
    for odd in ["", "-o", "a b", "a\nb", "a:b", "../x"] {
        assert!(!is_account_name(odd), "{odd:?} passed as an account name");
    }
}

use super::*;

fn server(name: &str, address: &str) -> Server {
    Server {
        name: ServerName::try_from(name).unwrap(),
        address: SshAddress::try_from(address).unwrap(),
        admin: None,
    }
}

#[test]
fn removing_default_clears_it() {
    let mut list = ServerList::default();
    list.add(server("a", "riku@a.local")).unwrap();
    let name = ServerName::try_from("a").unwrap();
    list.set_default(&name).unwrap();
    assert!(list.default_server().is_some());
    list.remove(&name).unwrap();
    assert!(list.default_server().is_none(), "default survived removal");
    assert!(list.get(&name).is_none());
}

#[test]
fn duplicate_name_is_rejected() {
    let mut list = ServerList::default();
    list.add(server("a", "riku@a.local")).unwrap();
    assert!(list.add(server("a", "riku@b.local")).is_err());
    assert_eq!(list.iter().count(), 1);
}

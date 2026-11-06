use super::*;

#[test]
fn a_copy_socket_is_not_the_single_socket_of_a_dotted_app() {
    let copy_of_shop = file_name("shop", 1);
    let single_of_shop_1 = "shop.1.sock";

    assert_ne!(copy_of_shop, single_of_shop_1);
    assert_eq!(parse(&copy_of_shop), Some(("shop", 1)));
    assert_eq!(
        parse(single_of_shop_1),
        None,
        "destroying app shop.1 must not read its own socket as a copy of shop"
    );
}

use super::*;

#[test]
fn echo_maps_old_prefixes_onto_the_rail() {
    assert_eq!(
        echo_mark("-----> Deploying app 'shop'", "green"),
        (Mark::Step, "Deploying app 'shop'")
    );
    assert_eq!(
        echo_mark("       https://shop.example", "green"),
        (Mark::Rail, "https://shop.example"),
        "an indented line is a detail under the step above"
    );
    assert_eq!(
        echo_mark("    riku apps create myapp", "yellow"),
        (Mark::Rail, "    riku apps create myapp"),
        "a deeper indent than the old one is kept"
    );
    assert_eq!(
        echo_mark("disk is low", "yellow"),
        (Mark::Warn, "disk is low")
    );
}

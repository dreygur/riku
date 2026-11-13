use super::*;

const ID: &str = "ab3fe4defd29ba6231229a4d41440ac8bde8218e85870e53876277faa24b35c4";

/// Podman 5 lists images as `sha256:<hex>` but puts the bare hex in a
/// container's `Image`, so comparing the strings as they come would replace
/// the service on every watch.
#[test]
fn a_copy_on_the_pulled_image_is_unchanged_whatever_prefix_the_engine_prints() {
    let pulled = format!("sha256:{ID}");

    assert!(runs_pulled_image(&[ID.to_string()], &pulled));
    assert!(runs_pulled_image(std::slice::from_ref(&pulled), &pulled));
    assert!(
        !runs_pulled_image(&[ID.to_string(), "sha256:0123".to_string()], &pulled),
        "one copy on another image means the tag moved"
    );
}

use crate::git::repo::post_receive_hook;
use crate::util::ROOT_BINARY;

#[test]
fn hook_names_only_the_root_binary() {
    let script = post_receive_hook();

    assert!(script.contains(ROOT_BINARY), "hook omits {ROOT_BINARY}");
    assert!(
        !script.contains(".local/bin")
            && !script.contains("command -v")
            && !script.contains("RIKU_BIN"),
        "hook looks for riku outside {ROOT_BINARY}:\n{script}"
    );
}

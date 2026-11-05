use super::*;

use crate::apps::create::app_hook_script;
use crate::git::repo::POST_RECEIVE_HOOK;
use crate::util::ROOT_BINARY;

#[test]
fn hooks_name_root_binary() {
    let app_hook = app_hook_script("myapp");
    let hooks = [
        ("global hook", GLOBAL_HOOK),
        ("app hook", app_hook.as_str()),
        ("receive-pack hook", POST_RECEIVE_HOOK),
    ];

    for (name, script) in hooks {
        assert!(script.contains(ROOT_BINARY), "{name} omits {ROOT_BINARY}");
        assert!(
            !script.contains(".local/bin")
                && !script.contains("command -v")
                && !script.contains("RIKU_BIN"),
            "{name} looks for riku outside {ROOT_BINARY}:\n{script}"
        );
    }
}

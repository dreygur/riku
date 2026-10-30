use super::*;
use crate::lock::LOCK_SUFFIX;

#[test]
fn sync_siblings_can_never_be_app_names() {
    // apps/ holds every app side by side. A sibling name an app could have
    // would let recovery for `myapp` delete that app, or its lock block it.
    let live = Path::new("/srv/riku/apps/myapp");
    for suffix in [STAGING_SUFFIX, PARKED_SUFFIX, LOCK_SUFFIX] {
        let path = sibling(live, suffix).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            riku_util::validation::validate_app_name(name).is_err(),
            "sync sibling {name:?} is a valid app name"
        );
    }
}

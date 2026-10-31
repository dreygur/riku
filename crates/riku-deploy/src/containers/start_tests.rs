use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;

use riku_container::types::CopyIndex;

use super::*;
use crate::containers::mock_engine::{logged, mock_engine, project_in};
use crate::containers::{copy_spec, AppName};

const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn spec_of(app: &str, project: &riku_container::compose::ComposeProject) -> RunSpec {
    copy_spec(
        &AppName::try_from(app).unwrap(),
        project,
        &project.services[0],
        CopyIndex::new(1).unwrap(),
        None,
        &BTreeMap::new(),
    )
    .unwrap()
}

#[test]
fn name_owned_by_other_app_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (app_path, project) =
        project_in(dir.path(), "a-b", "services:\n  c:\n    image: nginx:1\n");
    let inspect = format!(
        r#"[{{"Id":"{ID}","Name":"/a-b-c-1","Image":"sha256:x","Config":{{"Image":"nginx:1","Labels":{{"riku.app":"a"}}}},"State":{{"Status":"running"}}}}]"#
    );
    let behaviour = format!("case \"$1\" in ps) echo {ID};; inspect) echo '{inspect}';; esac");
    let (engine, log) = mock_engine(dir.path(), &behaviour);
    let spec = spec_of("a-b", &project);

    let err = start_copy(&engine, &spec, &app_path, dir.path()).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("`a`") && message.contains("`a-b`"),
        "{message}"
    );
    assert!(
        logged(&log).iter().all(|l| !l.starts_with("run")),
        "engine run was called: {:?}",
        logged(&log)
    );
}

#[test]
fn bind_through_new_symlink_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let compose = "
services:
  web:
    image: nginx:1
    volumes: [\"./data:/data\"]
";
    let (app_path, project) = {
        let app = dir.path().join("myapp");
        fs::create_dir_all(app.join("data")).unwrap();
        project_in(dir.path(), "myapp", compose)
    };
    let (engine, log) = mock_engine(dir.path(), "");
    let spec = spec_of("myapp", &project);
    fs::remove_dir(app_path.join("data")).unwrap();
    symlink("/", app_path.join("data")).unwrap();

    let err = start_copy(&engine, &spec, &app_path, dir.path()).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("data") && message.contains("symlink"),
        "{message}"
    );
    assert!(
        logged(&log).iter().all(|l| !l.starts_with("run")),
        "engine run was called: {:?}",
        logged(&log)
    );
}

#[test]
fn free_name_starts_the_container() {
    let dir = tempfile::tempdir().unwrap();
    let (app_path, project) = project_in(
        dir.path(),
        "myapp",
        "services:\n  web:\n    image: nginx:1\n",
    );
    let behaviour = format!("case \"$1\" in run) echo {ID};; esac");
    let (engine, log) = mock_engine(dir.path(), &behaviour);
    let spec = spec_of("myapp", &project);

    let id = start_copy(&engine, &spec, &app_path, dir.path()).unwrap();

    assert_eq!(id.as_str(), ID);
    assert!(logged(&log).iter().any(|l| l.starts_with("run")));
}

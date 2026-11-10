use super::*;
use std::fs;
use tempfile::TempDir;

fn app_with(file: &str) -> TempDir {
    let app = TempDir::new().unwrap();
    fs::write(app.path().join(file), "").unwrap();
    app
}

#[test]
fn markers_pick_the_runtime() {
    let cases = [
        ("go.mod", "go"),
        ("main.go", "go"),
        ("package.json", "node"),
        ("requirements.txt", "python"),
        ("pyproject.toml", "python"),
        ("setup.py", "python"),
        ("Gemfile", "ruby"),
        ("Cargo.toml", "rust-lang"),
    ];
    for (file, runtime) in cases {
        let app = app_with(file);
        assert_eq!(needed_runtime(app.path()), Some(runtime), "marker {file}");
    }
}

#[test]
fn first_marker_in_order_wins() {
    let app = app_with("package.json");
    fs::write(app.path().join("requirements.txt"), "").unwrap();
    assert_eq!(needed_runtime(app.path()), Some("node"));
}

#[test]
fn no_marker_gives_none() {
    let app = app_with("README.md");
    assert_eq!(needed_runtime(app.path()), None);
}

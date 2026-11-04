use super::*;

#[test]
fn python_scaffold_has_procfile_and_detectable_marker() {
    let files = Runtime::Python.files("demo");
    let names: Vec<&str> = files.iter().map(|f| f.path).collect();
    assert!(names.contains(&"Procfile"));
    assert!(names.contains(&"requirements.txt")); // buildpack detection
    assert!(names.contains(&"app.py"));
    let procfile = &files
        .iter()
        .find(|f| f.path == "Procfile")
        .unwrap()
        .contents;
    assert!(procfile.contains("python3 app.py"));
}

#[test]
fn node_scaffold_has_package_json_and_procfile() {
    let files = Runtime::Node.files("demo");
    let names: Vec<&str> = files.iter().map(|f| f.path).collect();
    assert!(names.contains(&"package.json"));
    assert!(names.contains(&"server.js"));
    let pkg = &files
        .iter()
        .find(|f| f.path == "package.json")
        .unwrap()
        .contents;
    assert!(pkg.contains("\"name\": \"demo\""));
}

#[test]
fn parse_accepts_aliases_and_rejects_unknown() {
    assert!(matches!(Runtime::parse("py").unwrap(), Runtime::Python));
    assert!(matches!(Runtime::parse("NODE").unwrap(), Runtime::Node));
    assert!(Runtime::parse("ruby").is_err());
}

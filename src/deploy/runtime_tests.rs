//! Tests for runtime detection.

#[cfg(test)]
mod tests {
    use crate::deploy::runtime::{detect_runtime, Runtime};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_marker(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    #[test]
    fn test_detect_python_requirements() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "requirements.txt");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Python));
    }

    #[test]
    fn test_detect_pyproject_fallback_to_python() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "pyproject.toml");
        let rt = detect_runtime(tmp.path());
        assert!(rt.is_some());
        match rt.unwrap() {
            Runtime::Python | Runtime::PythonPoetry | Runtime::PythonUv => {}
            other => panic!("Expected a Python variant, got {:?}", other),
        }
    }

    #[test]
    fn test_detect_ruby() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Gemfile");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Ruby));
    }

    #[test]
    fn test_detect_node() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "package.json");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Node));
    }

    #[test]
    fn test_detect_java_maven() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "pom.xml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::JavaMaven));
    }

    #[test]
    fn test_detect_java_gradle() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "build.gradle");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::JavaGradle));
    }

    #[test]
    fn test_detect_go_godeps() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Godeps");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_go_mod() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "go.mod");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_go_files() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "main.go");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Go));
    }

    #[test]
    fn test_detect_clojure_cli() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "deps.edn");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::ClojureCli));
    }

    #[test]
    fn test_detect_clojure_lein() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "project.clj");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::ClojureLein));
    }

    #[test]
    fn test_detect_rust() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Cargo.toml");
        create_marker(tmp.path(), "rust-toolchain.toml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Rust));
    }

    #[test]
    fn test_detect_rust_needs_both_files() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Cargo.toml");
        assert_eq!(detect_runtime(tmp.path()), None);
    }

    #[test]
    fn test_detect_no_runtime() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_runtime(tmp.path()), None);
    }

    #[test]
    fn test_priority_requirements_over_pyproject() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "requirements.txt");
        create_marker(tmp.path(), "pyproject.toml");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Python));
    }

    #[test]
    fn test_priority_gemfile_over_package_json() {
        let tmp = TempDir::new().unwrap();
        create_marker(tmp.path(), "Gemfile");
        create_marker(tmp.path(), "package.json");
        assert_eq!(detect_runtime(tmp.path()), Some(Runtime::Ruby));
    }

    #[test]
    fn test_runtime_display() {
        assert_eq!(Runtime::Python.to_string(), "Python");
        assert_eq!(Runtime::PythonPoetry.to_string(), "Python (Poetry)");
        assert_eq!(Runtime::PythonUv.to_string(), "Python (uv)");
        assert_eq!(Runtime::Node.to_string(), "Node");
        assert_eq!(Runtime::Go.to_string(), "Go");
        assert_eq!(Runtime::Rust.to_string(), "Rust");
    }
}

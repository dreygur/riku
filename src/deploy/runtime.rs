//! Runtime detection for deployed applications.
//!
//! Identifies the application runtime by inspecting marker files in the app directory.

use std::fs;
use std::path::Path;
use which::which;

/// Supported application runtimes, detected from marker files.
#[derive(Debug, PartialEq)]
pub enum Runtime {
    Python,
    PythonPoetry,
    PythonUv,
    Node,
    Ruby,
    Go,
    Rust,
    JavaMaven,
    JavaGradle,
    ClojureCli,
    ClojureLein,
    Container,
    Identity,
    Wsgi,
    Jwsgi,
    Rwsgi,
    Php,
}

impl std::fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runtime::Python => write!(f, "Python"),
            Runtime::PythonPoetry => write!(f, "Python (Poetry)"),
            Runtime::PythonUv => write!(f, "Python (uv)"),
            Runtime::Node => write!(f, "Node"),
            Runtime::Ruby => write!(f, "Ruby"),
            Runtime::Go => write!(f, "Go"),
            Runtime::Rust => write!(f, "Rust"),
            Runtime::JavaMaven => write!(f, "Java Maven"),
            Runtime::JavaGradle => write!(f, "Java Gradle"),
            Runtime::ClojureCli => write!(f, "Clojure CLI"),
            Runtime::ClojureLein => write!(f, "Clojure Lein"),
            Runtime::Container => write!(f, "Container"),
            Runtime::Identity => write!(f, "Identity"),
            Runtime::Wsgi => write!(f, "Python WSGI"),
            Runtime::Jwsgi => write!(f, "Java WSGI"),
            Runtime::Rwsgi => write!(f, "Ruby WSGI"),
            Runtime::Php => write!(f, "PHP"),
        }
    }
}

/// Detect the application runtime by checking marker files in the app directory.
pub fn detect_runtime(app_path: &Path) -> Option<Runtime> {
    // 1. requirements.txt -> Python
    if app_path.join("requirements.txt").exists() {
        return Some(Runtime::Python);
    }

    // 2-4. pyproject.toml with poetry/uv/fallback
    if app_path.join("pyproject.toml").exists() {
        if which("poetry").is_ok() {
            return Some(Runtime::PythonPoetry);
        }
        if which("uv").is_ok() {
            return Some(Runtime::PythonUv);
        }
        // fallback: plain Python
        return Some(Runtime::Python);
    }

    // 5. Gemfile -> Ruby
    if app_path.join("Gemfile").exists() {
        return Some(Runtime::Ruby);
    }

    // 6. package.json -> Node
    if app_path.join("package.json").exists() {
        return Some(Runtime::Node);
    }

    // 7. pom.xml -> JavaMaven
    if app_path.join("pom.xml").exists() {
        return Some(Runtime::JavaMaven);
    }

    // 8. build.gradle -> JavaGradle
    if app_path.join("build.gradle").exists() {
        return Some(Runtime::JavaGradle);
    }

    // 9. Godeps or go.mod or *.go files -> Go
    if app_path.join("Godeps").exists() || app_path.join("go.mod").exists() {
        return Some(Runtime::Go);
    }
    if let Ok(entries) = fs::read_dir(app_path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "go" {
                    return Some(Runtime::Go);
                }
            }
        }
    }

    // 10. deps.edn -> ClojureCli
    if app_path.join("deps.edn").exists() {
        return Some(Runtime::ClojureCli);
    }

    // 11. project.clj -> ClojureLein
    if app_path.join("project.clj").exists() {
        return Some(Runtime::ClojureLein);
    }

    // 12. Dockerfile or Containerfile -> Container
    if app_path.join("Dockerfile").exists() || app_path.join("Containerfile").exists() {
        return Some(Runtime::Container);
    }

    // 13. docker-compose.yml or podman-compose.yml -> Container
    if app_path.join("docker-compose.yml").exists()
        || app_path.join("docker-compose.yaml").exists()
        || app_path.join("podman-compose.yml").exists()
        || app_path.join("podman-compose.yaml").exists()
        || app_path.join("compose.yml").exists()
        || app_path.join("compose.yaml").exists()
    {
        return Some(Runtime::Container);
    }

    // 16. Cargo.toml + rust-toolchain.toml -> Rust
    if app_path.join("Cargo.toml").exists() && app_path.join("rust-toolchain.toml").exists() {
        return Some(Runtime::Rust);
    }

    // 17. Check Procfile for wsgi/jwsgi/rwsgi/php workers
    let procfile_path = app_path.join("Procfile");
    if procfile_path.exists() {
        if let Ok(content) = fs::read_to_string(&procfile_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(pos) = line.find(':') {
                    let kind = line[..pos].trim();
                    match kind {
                        "wsgi" | "jwsgi" | "rwsgi" | "php" => {
                            // Check if corresponding marker file exists
                            match kind {
                                "wsgi" => {
                                    // WSGI needs Python app
                                    if app_path.join("requirements.txt").exists()
                                        || app_path.join("pyproject.toml").exists()
                                        || app_path.join("wsgi.py").exists()
                                    {
                                        return Some(Runtime::Wsgi);
                                    }
                                }
                                "jwsgi" => {
                                    // JWSGI needs Java
                                    if app_path.join("pom.xml").exists()
                                        || app_path.join("build.gradle").exists()
                                    {
                                        return Some(Runtime::Jwsgi);
                                    }
                                }
                                "rwsgi" => {
                                    // RWSGI needs Ruby
                                    if app_path.join("Gemfile").exists() {
                                        return Some(Runtime::Rwsgi);
                                    }
                                }
                                "php" => {
                                    // PHP just needs the php worker
                                    return Some(Runtime::Php);
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 18. No runtime detected
    None
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

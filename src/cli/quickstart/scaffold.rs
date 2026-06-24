//! Sample-app scaffolds for `riku quickstart`.
//!
//! Each runtime produces a minimal, dependency-free web app that binds to
//! `$PORT` (the variable Riku injects) and a `Procfile` Riku deploys as-is. The
//! presence of `requirements.txt` / `package.json` is what the runtime
//! buildpacks detect, so the scaffolded app deploys without further setup.

/// A file to write into the scaffolded app directory.
pub struct ScaffoldFile {
    pub path: &'static str,
    pub contents: String,
}

/// Runtimes `quickstart` can scaffold.
pub enum Runtime {
    Python,
    Node,
}

impl Runtime {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "python" | "py" => Ok(Runtime::Python),
            "node" | "nodejs" | "js" => Ok(Runtime::Node),
            other => anyhow::bail!("unknown runtime '{other}' (supported: python, node)"),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Runtime::Python => "Python",
            Runtime::Node => "Node.js",
        }
    }

    /// The files to write for an app named `app`.
    pub fn files(&self, app: &str) -> Vec<ScaffoldFile> {
        match self {
            Runtime::Python => python_files(app),
            Runtime::Node => node_files(app),
        }
    }
}

fn python_files(app: &str) -> Vec<ScaffoldFile> {
    vec![
        ScaffoldFile {
            path: "Procfile",
            contents: "web: python3 app.py\n".to_string(),
        },
        ScaffoldFile {
            // Presence triggers the python buildpack; empty of real deps.
            path: "requirements.txt",
            contents: "# Add your dependencies here, one per line.\n".to_string(),
        },
        ScaffoldFile {
            path: "app.py",
            contents: PYTHON_APP.to_string(),
        },
        ScaffoldFile {
            path: ".gitignore",
            contents: "__pycache__/\n*.pyc\n.venv/\n".to_string(),
        },
        ScaffoldFile {
            path: "README.md",
            contents: readme(app, "Python", "python3 app.py"),
        },
    ]
}

fn node_files(app: &str) -> Vec<ScaffoldFile> {
    vec![
        ScaffoldFile {
            path: "Procfile",
            contents: "web: node server.js\n".to_string(),
        },
        ScaffoldFile {
            // Presence triggers the node buildpack.
            path: "package.json",
            contents: format!(
                "{{\n  \"name\": \"{app}\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"scripts\": {{ \"start\": \"node server.js\" }}\n}}\n"
            ),
        },
        ScaffoldFile {
            path: "server.js",
            contents: NODE_APP.to_string(),
        },
        ScaffoldFile {
            path: ".gitignore",
            contents: "node_modules/\nnpm-debug.log\n".to_string(),
        },
        ScaffoldFile {
            path: "README.md",
            contents: readme(app, "Node.js", "node server.js"),
        },
    ]
}

fn readme(app: &str, runtime: &str, start: &str) -> String {
    format!(
        "# {app}\n\nA sample {runtime} app scaffolded by `riku quickstart`.\n\n\
         Runs `{start}`, listening on `$PORT`.\n\n\
         Deploy with `git push riku main` (see the quickstart output).\n"
    )
}

const PYTHON_APP: &str = r#"import os
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.end_headers()
        self.wfile.write(b"Hello from Riku!\n")

    def log_message(self, *_args):
        pass  # quiet access logs


if __name__ == "__main__":
    port = int(os.environ.get("PORT", "8000"))
    print(f"listening on :{port}", flush=True)
    HTTPServer(("0.0.0.0", port), Handler).serve_forever()
"#;

const NODE_APP: &str = r#"const http = require("http");

const port = process.env.PORT || 8000;

http
  .createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    res.end("Hello from Riku!\n");
  })
  .listen(port, "0.0.0.0", () => console.log(`listening on :${port}`));
"#;

#[cfg(test)]
mod tests {
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
}

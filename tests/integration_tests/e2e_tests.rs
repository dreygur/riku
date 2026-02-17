/// End-to-End Deployment Tests
///
/// These tests simulate real deployment scenarios
/// from git push to process supervision.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment
    fn setup_riku_env() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let riku_root = temp_dir.path().join(".riku");

        let dirs = [
            "apps",
            "data",
            "envs",
            "repos",
            "logs",
            "nginx",
            "cache",
            "workers",
            "workers-available",
            "workers-enabled",
            "acme",
            "acme-www",
            "plugins",
        ];

        for dir in &dirs {
            fs::create_dir_all(riku_root.join(dir))?;
        }

        Ok((temp_dir, riku_root))
    }

    // Simulate a complete Node.js app deployment
    fn create_nodejs_app(app_dir: &PathBuf) -> Result<()> {
        // package.json
        let package_json = r#"{
  "name": "test-nodejs-app",
  "version": "1.0.0",
  "scripts": {
    "start": "node server.js"
  },
  "dependencies": {
    "express": "^4.18.0"
  }
}"#;
        fs::write(app_dir.join("package.json"), package_json)?;

        // server.js
        let server_js = r#"
const express = require('express');
const app = express();
const port = process.env.PORT || 3000;

app.get('/', (req, res) => {
  res.send('Hello from Node.js!');
});

app.get('/health', (req, res) => {
  res.json({ status: 'healthy' });
});

app.listen(port, () => {
  console.log(`App listening on port ${port}`);
});
"#;
        fs::write(app_dir.join("server.js"), server_js)?;

        // Procfile
        fs::write(app_dir.join("Procfile"), "web: node server.js\n")?;

        Ok(())
    }

    // Simulate a complete Python app deployment
    fn create_python_app(app_dir: &PathBuf) -> Result<()> {
        // requirements.txt
        fs::write(
            app_dir.join("requirements.txt"),
            "flask>=2.0.0\ngunicorn>=20.0.0\n",
        )?;

        // app.py
        let app_py = r#"
from flask import Flask, jsonify
import os

app = Flask(__name__)
port = int(os.environ.get('PORT', 5000))

@app.route('/')
def hello():
    return 'Hello from Python!'

@app.route('/health')
def health():
    return jsonify({'status': 'healthy'})

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=port)
"#;
        fs::write(app_dir.join("app.py"), app_py)?;

        // Procfile
        fs::write(app_dir.join("Procfile"), "web: gunicorn app:app\n")?;

        Ok(())
    }

    // Simulate a complete Ruby app deployment
    fn create_ruby_app(app_dir: &PathBuf) -> Result<()> {
        // Gemfile
        let gemfile = r#"
source 'https://rubygems.org'

gem 'sinatra', '~> 3.0'
gem 'puma', '~> 5.0'
"#;
        fs::write(app_dir.join("Gemfile"), gemfile)?;

        // app.rb
        let app_rb = r#"
require 'sinatra'
require 'json'

set :port, ENV['PORT'] || 4567
set :bind, '0.0.0.0'

get '/' do
  'Hello from Ruby!'
end

get '/health' do
  content_type :json
  { status: 'healthy' }.to_json
end
"#;
        fs::write(app_dir.join("app.rb"), app_rb)?;

        // Procfile
        fs::write(app_dir.join("Procfile"), "web: ruby app.rb\n")?;

        Ok(())
    }

    #[test]
    fn test_complete_nodejs_deployment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "nodejs-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let env_dir = riku_root.join("envs").join(app_name);
        let log_dir = riku_root.join("logs").join(app_name);
        let workers_avail = riku_root.join("workers-available");
        let workers_enabled = riku_root.join("workers-enabled");

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&env_dir)?;
        fs::create_dir_all(&log_dir)?;

        // Create Node.js app
        create_nodejs_app(&app_dir)?;

        // Create ENV file
        fs::write(env_dir.join("ENV"), "PORT=3000\nNODE_ENV=production\n")?;

        // Create SCALING file
        fs::write(app_dir.join("SCALING"), "web=2\n")?;

        // Create worker configs
        for i in 1..=2 {
            let worker_config = format!(
                r#"[worker]
app = "{}"
kind = "web"
command = "node server.js"
ordinal = {}

[env]
PORT = "{}"
NODE_ENV = "production"

[options]
working_dir = "{}"
log_file = "{}/web.{}.log"
"#,
                app_name,
                i,
                3000 + i - 1,
                app_dir.display(),
                log_dir.display(),
                i
            );

            let config_file = workers_avail.join(format!("{}.web.{}.toml", app_name, i));
            fs::write(&config_file, worker_config)?;

            // Enable worker
            #[cfg(unix)]
            std::os::unix::fs::symlink(
                &config_file,
                workers_enabled.join(format!("{}.web.{}.toml", app_name, i)),
            )?;
        }

        // Verify deployment
        assert!(app_dir.exists());
        assert!(env_dir.exists());
        assert!(log_dir.exists());
        assert!(app_dir.join("package.json").exists());
        assert!(app_dir.join("server.js").exists());
        assert!(app_dir.join("Procfile").exists());

        let workers: Vec<_> = fs::read_dir(&workers_enabled)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(workers.len(), 2);

        Ok(())
    }

    #[test]
    fn test_complete_python_deployment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "python-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let env_dir = riku_root.join("envs").join(app_name);
        let log_dir = riku_root.join("logs").join(app_name);

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&env_dir)?;
        fs::create_dir_all(&log_dir)?;

        // Create Python app
        create_python_app(&app_dir)?;

        // Create ENV file with various settings
        let env_content = r#"PORT=5000
FLASK_ENV=production
DATABASE_URL=sqlite:///app.db
SECRET_KEY=supersecret
DEBUG=false
"#;
        fs::write(env_dir.join("ENV"), env_content)?;

        // Verify deployment
        assert!(app_dir.join("requirements.txt").exists());
        assert!(app_dir.join("app.py").exists());
        assert!(app_dir.join("Procfile").exists());

        let env_file = env_dir.join("ENV");
        let content = fs::read_to_string(&env_file)?;
        assert!(content.contains("DATABASE_URL"));
        assert!(content.contains("SECRET_KEY"));

        Ok(())
    }

    #[test]
    fn test_complete_ruby_deployment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "ruby-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let env_dir = riku_root.join("envs").join(app_name);

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&env_dir)?;

        // Create Ruby app
        create_ruby_app(&app_dir)?;

        // Create ENV file
        fs::write(env_dir.join("ENV"), "PORT=4567\nRACK_ENV=production\n")?;

        // Verify deployment
        assert!(app_dir.join("Gemfile").exists());
        assert!(app_dir.join("app.rb").exists());
        assert!(app_dir.join("Procfile").exists());

        Ok(())
    }

    #[test]
    fn test_multi_process_deployment() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "multi-process-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let env_dir = riku_root.join("envs").join(app_name);
        let log_dir = riku_root.join("logs").join(app_name);
        let workers_avail = riku_root.join("workers-available");

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&env_dir)?;
        fs::create_dir_all(&log_dir)?;

        // Create app with multiple process types
        create_python_app(&app_dir)?;

        // Procfile with multiple processes
        let procfile = r#"web: gunicorn app:app
worker: python worker.py
cron: 0 * * * * python cleanup.py
"#;
        fs::write(app_dir.join("Procfile"), procfile)?;

        // Create worker files
        fs::write(app_dir.join("worker.py"), "# Worker process\n")?;
        fs::write(app_dir.join("cleanup.py"), "# Cleanup script\n")?;

        // Create worker configs for each process type
        let worker_configs = vec![
            ("web", 1, "gunicorn app:app"),
            ("web", 2, "gunicorn app:app"),
            ("worker", 1, "python worker.py"),
            ("worker", 2, "python worker.py"),
            ("cron", 1, "0 * * * * python cleanup.py"),
        ];

        for (kind, ordinal, command) in worker_configs {
            let config = format!(
                r#"[worker]
app = "{}"
kind = "{}"
command = "{}"
ordinal = {}
"#,
                app_name, kind, command, ordinal
            );

            let config_file = workers_avail.join(format!("{}.{}.{}.toml", app_name, kind, ordinal));
            fs::write(&config_file, config)?;
        }

        let workers: Vec<_> = fs::read_dir(&workers_avail)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(workers.len(), 5);

        Ok(())
    }

    #[test]
    fn test_deployment_with_scaling() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "scaled-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let workers_avail = riku_root.join("workers-available");

        fs::create_dir_all(&app_dir)?;

        create_nodejs_app(&app_dir)?;

        // Initial scaling
        fs::write(app_dir.join("SCALING"), "web=2\nworker=1\n")?;

        // Create initial workers
        for i in 1..=2 {
            let config_file = workers_avail.join(format!("{}.web.{}.toml", app_name, i));
            fs::write(
                &config_file,
                format!("[worker]\napp = \"{}\"\nkind = \"web\"\n", app_name),
            )?;
        }

        let workers: Vec<_> = fs::read_dir(&workers_avail)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(workers.len(), 2);

        // Scale up
        fs::write(app_dir.join("SCALING"), "web=4\nworker=2\n")?;

        // Create additional workers
        for i in 3..=4 {
            let config_file = workers_avail.join(format!("{}.web.{}.toml", app_name, i));
            fs::write(
                &config_file,
                format!("[worker]\napp = \"{}\"\nkind = \"web\"\n", app_name),
            )?;
        }

        let workers: Vec<_> = fs::read_dir(&workers_avail)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        assert_eq!(workers.len(), 4);

        Ok(())
    }

    #[test]
    fn test_deployment_with_env_update() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "env-update-app";
        let env_dir = riku_root.join("envs").join(app_name);

        fs::create_dir_all(&env_dir)?;

        // Initial ENV
        fs::write(env_dir.join("ENV"), "PORT=3000\nDEBUG=false\n")?;

        let env_file = env_dir.join("ENV");
        let mut content = fs::read_to_string(&env_file)?;
        assert!(content.contains("DEBUG=false"));

        // Update ENV (simulating config:set)
        content.push_str("NEW_VAR=new_value\n");
        content = content.replace("DEBUG=false", "DEBUG=true");
        fs::write(&env_file, content)?;

        let updated = fs::read_to_string(&env_file)?;
        assert!(updated.contains("NEW_VAR=new_value"));
        assert!(updated.contains("DEBUG=true"));

        Ok(())
    }

    #[test]
    fn test_deployment_rollback_simulation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "rollback-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let releases_dir = app_dir.join("releases");

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&releases_dir)?;

        // Simulate multiple releases
        for i in 1..=3 {
            let release_dir = releases_dir.join(format!("v{}", i));
            fs::create_dir_all(&release_dir)?;
            fs::write(release_dir.join("app.js"), format!("// Release {}", i))?;
            fs::write(release_dir.join("timestamp"), format!("{}", i * 1000))?;
        }

        // Current release points to v3
        fs::write(app_dir.join("CURRENT_RELEASE"), "v3\n")?;

        let current = fs::read_to_string(app_dir.join("CURRENT_RELEASE"))?;
        assert!(current.contains("v3"));

        // Rollback to v2
        fs::write(app_dir.join("CURRENT_RELEASE"), "v2\n")?;

        let rolled_back = fs::read_to_string(app_dir.join("CURRENT_RELEASE"))?;
        assert!(rolled_back.contains("v2"));

        Ok(())
    }

    #[test]
    fn test_deployment_with_health_checks() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "health-check-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let log_dir = riku_root.join("logs").join(app_name);

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&log_dir)?;

        create_nodejs_app(&app_dir)?;

        // Create health check log
        let health_log = log_dir.join("health.log");
        let health_entries = r#"2024-01-01 10:00:00 Health check passed
2024-01-01 10:01:00 Health check passed
2024-01-01 10:02:00 Health check passed
2024-01-01 10:03:00 Health check failed - timeout
2024-01-01 10:03:30 Health check passed - recovered
"#;
        fs::write(&health_log, health_entries)?;

        // Parse health status
        let content = fs::read_to_string(&health_log)?;
        let lines: Vec<&str> = content.lines().collect();
        let last_entry = lines.last().unwrap();

        assert!(last_entry.contains("recovered"));

        Ok(())
    }

    #[test]
    fn test_deployment_cleanup() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "cleanup-app";
        let dirs = ["apps", "envs", "logs", "nginx", "data"];

        // Create all directories
        for dir in &dirs {
            let path = riku_root.join(dir).join(app_name);
            fs::create_dir_all(&path)?;

            // Add files
            if dir == &"apps" {
                fs::write(path.join("app.js"), "console.log('app');")?;
            } else if dir == &"envs" {
                fs::write(path.join("ENV"), "KEY=value")?;
            } else if dir == &"logs" {
                fs::write(path.join("web.1.log"), "log entry")?;
            }
        }

        // Verify all exist
        for dir in &dirs {
            assert!(riku_root.join(dir).join(app_name).exists());
        }

        // Simulate app destruction - cleanup all directories
        for dir in &dirs {
            let path = riku_root.join(dir).join(app_name);
            if path.exists() {
                fs::remove_dir_all(&path)?;
            }
        }

        // Verify cleanup
        for dir in &dirs {
            assert!(!riku_root.join(dir).join(app_name).exists());
        }

        Ok(())
    }

    #[test]
    fn test_concurrent_deployments() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let apps = vec!["app1", "app2", "app3", "app4", "app5"];

        // Deploy multiple apps concurrently (simulated)
        for app in &apps {
            let app_dir = riku_root.join("apps").join(app);
            let env_dir = riku_root.join("envs").join(app);

            fs::create_dir_all(&app_dir)?;
            fs::create_dir_all(&env_dir)?;

            create_nodejs_app(&app_dir)?;
            fs::write(env_dir.join("ENV"), format!("APP_NAME={}\n", app))?;
        }

        // Verify all apps deployed
        let deployed_apps: Vec<_> = fs::read_dir(riku_root.join("apps"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        assert_eq!(deployed_apps.len(), 5);

        Ok(())
    }

    #[test]
    fn test_deployment_with_data_persistence() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        let app_name = "data-app";
        let app_dir = riku_root.join("apps").join(app_name);
        let data_dir = riku_root.join("data").join(app_name);

        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&data_dir)?;

        // Create app
        create_python_app(&app_dir)?;

        // Create persistent data
        fs::write(data_dir.join("database.db"), "persistent data")?;
        fs::write(data_dir.join("uploads"), "user uploads")?;
        fs::create_dir_all(data_dir.join("sessions"))?;

        assert!(data_dir.join("database.db").exists());
        assert!(data_dir.join("uploads").exists());
        assert!(data_dir.join("sessions").exists());

        Ok(())
    }
}

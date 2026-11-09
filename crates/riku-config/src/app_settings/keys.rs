//! The `riku.toml` key table: each `section.name` and the `RIKU_*` env key
//! riku reads today.
//!
//! The table is a transitional adapter. Consumers still read env keys; later
//! work gives the loader typed fields instead of adding env keys here.

/// How a value is typed in TOML and rendered into the env map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    /// A non-negative integer, rendered in decimal.
    Int,
    /// An array of strings, rendered joined with `,`.
    List,
    /// A bool whose reader checks presence or an on value: `true` renders
    /// `"true"`, `false` leaves the key out.
    Flag,
    /// A bool whose reader needs an explicit `"false"` (it defaults to on).
    Bool,
}

/// Which file may set a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Any,
    /// Only the server override; the repo file is committed and pushed by
    /// whoever can push, so it may not loosen limits or hold secrets.
    ServerOnly,
}

/// Which env key a table key renders to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvName {
    Fixed(&'static str),
    /// `RIKU_<RUNTIME NAME>_VERSION`, from `runtime.name`.
    RuntimeVersion,
}

/// One `riku.toml` key.
#[derive(Debug, PartialEq, Eq)]
pub struct Key {
    pub section: &'static str,
    pub name: &'static str,
    pub env: EnvName,
    pub kind: Kind,
    pub scope: Scope,
    /// Masked by `riku settings show`.
    pub secret: bool,
}

const fn key(section: &'static str, name: &'static str, env: &'static str, kind: Kind) -> Key {
    Key {
        section,
        name,
        env: EnvName::Fixed(env),
        kind,
        scope: Scope::Any,
        secret: false,
    }
}

const fn server(section: &'static str, name: &'static str, env: &'static str, kind: Kind) -> Key {
    Key {
        section,
        name,
        env: EnvName::Fixed(env),
        kind,
        scope: Scope::ServerOnly,
        secret: false,
    }
}

const fn secret(name: &'static str, env: &'static str) -> Key {
    Key {
        section: "secrets",
        name,
        env: EnvName::Fixed(env),
        kind: Kind::Text,
        scope: Scope::ServerOnly,
        secret: true,
    }
}

/// Every key `riku.toml` accepts.
pub const KEYS: &[Key] = &[
    key("routing", "domains", "RIKU_DOMAINS", Kind::List),
    key("routing", "https_only", "RIKU_HTTPS_ONLY", Kind::Flag),
    key(
        "routing",
        "document_root",
        "RIKU_NGINX_DOCUMENT_ROOT",
        Kind::Text,
    ),
    key("routing", "static", "RIKU_NGINX_STATIC", Kind::Flag),
    key(
        "routing",
        "static_paths",
        "RIKU_NGINX_STATIC_PATHS",
        Kind::List,
    ),
    key(
        "routing",
        "allow_git_folders",
        "RIKU_NGINX_ALLOW_GIT_FOLDERS",
        Kind::Flag,
    ),
    server("routing", "bind_address", "RIKU_BIND_ADDRESS", Kind::Text),
    key("runtime", "name", "RIKU_RUNTIME", Kind::Text),
    Key {
        section: "runtime",
        name: "version",
        env: EnvName::RuntimeVersion,
        kind: Kind::Text,
        scope: Scope::Any,
        secret: false,
    },
    key("workers", "timeout", "RIKU_WORKER_TIMEOUT", Kind::Int),
    key(
        "workers",
        "grace_period",
        "RIKU_WORKER_GRACE_PERIOD",
        Kind::Int,
    ),
    key("workers", "max_restarts", "RIKU_MAX_RESTARTS", Kind::Int),
    key("workers", "auto_restart", "RIKU_AUTO_RESTART", Kind::Bool),
    key("health", "url", "RIKU_HEALTH_CHECK_URL", Kind::Text),
    key(
        "health",
        "interval",
        "RIKU_HEALTH_CHECK_INTERVAL",
        Kind::Int,
    ),
    key("health", "timeout", "RIKU_HEALTH_CHECK_TIMEOUT", Kind::Int),
    key("health", "retries", "RIKU_HEALTH_CHECK_RETRIES", Kind::Int),
    server("limits", "max_memory_mb", "RIKU_MAX_MEMORY_MB", Kind::Int),
    server(
        "limits",
        "max_cpu_seconds",
        "RIKU_MAX_CPU_SECONDS",
        Kind::Int,
    ),
    server(
        "limits",
        "max_file_size_mb",
        "RIKU_MAX_FILE_SIZE_MB",
        Kind::Int,
    ),
    server("limits", "max_open_files", "RIKU_MAX_OPEN_FILES", Kind::Int),
    server("limits", "max_processes", "RIKU_MAX_PROCESSES", Kind::Int),
    server(
        "limits",
        "isolation_max_memory_mb",
        "RIKU_ISOLATION_MAX_MEMORY_MB",
        Kind::Int,
    ),
    server(
        "limits",
        "isolation_max_pids",
        "RIKU_ISOLATION_MAX_PIDS",
        Kind::Int,
    ),
    server(
        "limits",
        "isolation_cpu_quota_us",
        "RIKU_ISOLATION_CPU_QUOTA_US",
        Kind::Int,
    ),
    server(
        "limits",
        "isolation_cpu_period_us",
        "RIKU_ISOLATION_CPU_PERIOD_US",
        Kind::Int,
    ),
    key(
        "containers",
        "watch_services",
        "RIKU_WATCH_SERVICES",
        Kind::List,
    ),
    secret("ghcr_username", "RIKU_GHCR_USERNAME"),
    secret("ghcr_token", "RIKU_GHCR_TOKEN"),
    secret("notify_webhook_url", "RIKU_NOTIFY_WEBHOOK_URL"),
];

impl Key {
    /// `section.name`.
    pub fn path(&self) -> String {
        format!("{}.{}", self.section, self.name)
    }
}

/// The key at `section.name`.
pub fn find(path: &str) -> Option<&'static Key> {
    match path.split_once('.') {
        Some((section, name)) => KEYS.iter().find(|k| k.section == section && k.name == name),
        None => None,
    }
}

/// `RIKU_<NAME>_VERSION` for runtime `name`: uppercased, `-` becomes `_`.
pub fn runtime_version_env(name: &str) -> String {
    format!(
        "RIKU_{}_VERSION",
        name.to_ascii_uppercase().replace('-', "_")
    )
}

/// The `section.name` whose fixed env key is `env`.
pub fn path_for_env(env: &str) -> Option<String> {
    KEYS.iter()
        .find(|k| matches!(k.env, EnvName::Fixed(name) if name == env))
        .map(Key::path)
}

/// Whether `env` is a fixed env key of the table (runtime version keys are
/// derived, so they are not included).
pub fn is_table_env_key(env: &str) -> bool {
    KEYS.iter()
        .any(|k| matches!(k.env, EnvName::Fixed(name) if name == env))
}

//! Marketplace index parsing — repository layer.
//!
//! A marketplace is a git repo with a `marketplace.toml` listing its plugins
//! and where each plugin's bundle is fetched from (`source`). The index is
//! read on `search`; the bundle payload is only pulled on `add` (progressive
//! disclosure).

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// One plugin advertised by a marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MarketplaceEntry {
    pub name: String,
    /// Install source: a git URL (`github:owner/repo`, `https://…`) or a path
    /// relative to the marketplace repo.
    pub source: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub plugin_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct IndexDoc {
    #[serde(default, rename = "plugin")]
    plugins: Vec<MarketplaceEntry>,
}

/// Parse the `marketplace.toml` in a marketplace clone directory.
pub fn read_index(dir: &Path) -> Result<Vec<MarketplaceEntry>> {
    let path = dir.join("marketplace.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading marketplace index {}", path.display()))?;
    let doc: IndexDoc =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(doc.plugins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugin_entries() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("marketplace.toml"),
            r#"
            [[plugin]]
            name = "postgres"
            source = "github:riku-plugins/postgres"
            description = "Managed PostgreSQL addon"
            type = "addon"

            [[plugin]]
            name = "redis"
            source = "github:riku-plugins/redis"
            "#,
        )
        .unwrap();

        let entries = read_index(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "postgres");
        assert_eq!(entries[0].source, "github:riku-plugins/postgres");
        assert_eq!(entries[0].plugin_type.as_deref(), Some("addon"));
        assert!(entries[1].description.is_none());
    }

    #[test]
    fn missing_index_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_index(tmp.path()).is_err());
    }
}

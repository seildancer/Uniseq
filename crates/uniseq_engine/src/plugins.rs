//! Plugin manifest scanner and capability validation.
//!
//! Scans app/plugins/*.toml and plugins/*.toml for plugin manifests,
//! validates capabilities, and provides a registry.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
}

/// Capability tokens a plugin may declare.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    ReadPages,
    WritePages,
    ReadJournal,
    WriteJournal,
    ManageAssets,
    QueryIndex,
    ReadSettings,
    WriteSettings,
    GraphView,
    SyncStatus,
}

impl Capability {
    pub fn from_str(s: &str) -> Option<Capability> {
        match s {
            "read-pages" => Some(Capability::ReadPages),
            "write-pages" => Some(Capability::WritePages),
            "read-journal" => Some(Capability::ReadJournal),
            "write-journal" => Some(Capability::WriteJournal),
            "manage-assets" => Some(Capability::ManageAssets),
            "query-index" => Some(Capability::QueryIndex),
            "read-settings" => Some(Capability::ReadSettings),
            "write-settings" => Some(Capability::WriteSettings),
            "graph-view" => Some(Capability::GraphView),
            "sync-status" => Some(Capability::SyncStatus),
            _ => None,
        }
    }
}

/// A single plugin manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Optional description.
    pub description: Option<String>,
    /// Declared capabilities.
    pub capabilities: Vec<Capability>,
    /// Entrypoint script or module (optional, for future use).
    pub entry: Option<String>,
    /// Whether the plugin is disabled.
    pub disabled: bool,
}

impl PluginManifest {
    /// Validate capabilities: ensure all are recognized tokens.
    /// Returns Ok if valid, Err with first bad token if invalid.
    pub fn validate(&self) -> Result<(), String> {
        for cap in &self.capabilities {
            let valid = matches!(cap,
                Capability::ReadPages | Capability::WritePages |
                Capability::ReadJournal | Capability::WriteJournal |
                Capability::ManageAssets | Capability::QueryIndex |
                Capability::ReadSettings | Capability::WriteSettings |
                Capability::GraphView | Capability::SyncStatus
            );
            if !valid {
                return Err(format!("unknown capability: {:?}", cap));
            }
        }
        Ok(())
    }
}

/// Registry of discovered plugins.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginRegistry {
    pub plugins: Vec<PluginManifest>,
}

/// Scan plugin directories and build a registry.
pub fn scan_plugins(root: impl AsRef<Path>) -> Result<PluginRegistry, PluginError> {
    let root = root.as_ref();
    let mut plugins = Vec::new();

    let plugin_dirs = ["app/plugins", "plugins"];
    for dir_name in &plugin_dirs {
        let dir = root.join(dir_name);
        if !dir.is_dir() { continue; }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
            let text = fs::read_to_string(&path)?;
            match toml::from_str::<toml::Value>(&text) {
                Ok(value) => {
                    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
                    let version = value.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
                    let description = value.get("description").and_then(|v| v.as_str()).map(String::from);
                    let entry = value.get("entry").and_then(|v| v.as_str()).map(String::from);
                    let disabled = value.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);

                    let caps_raw = value.get("capabilities")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>()
                        })
                        .unwrap_or_default();

                    // Deny-by-default: any unknown capability string causes manifest rejection.
                    let mut caps = Vec::new();
                    let mut has_unknown = false;
                    for cap_str in &caps_raw {
                        match Capability::from_str(cap_str) {
                            Some(c) => caps.push(c),
                            None => {
                                // Unknown capability — reject this plugin
                                has_unknown = true;
                                break;
                            }
                        }
                    }

                    // Only add if all capabilities were valid
                    if !has_unknown && caps.len() == caps_raw.len() {
                        let manifest = PluginManifest { id: id.clone(), name, version, description, capabilities: caps, entry, disabled };
                        if manifest.validate().is_ok() {
                            plugins.push(manifest);
                        }
                    }
                }
                Err(_) => {
                    // Skip malformed manifests — log would be nice but we keep it quiet
                }
            }
        }
    }

    Ok(PluginRegistry { plugins })
}

/// Check whether a registry has a specific capability across any plugin.
pub fn has_capability(registry: &PluginRegistry, cap: Capability) -> bool {
    registry.plugins.iter().any(|p| !p.disabled && p.capabilities.contains(&cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_from_str_valid() {
        assert_eq!(Capability::from_str("read-pages"), Some(Capability::ReadPages));
        assert_eq!(Capability::from_str("write-journal"), Some(Capability::WriteJournal));
    }

    #[test]
    fn capability_from_str_invalid() {
        assert_eq!(Capability::from_str("do-anything"), None);
    }

    #[test]
    fn manifest_validate_good() {
        let m = PluginManifest {
            id: "test".into(),
            name: "Test".into(),
            version: "1.0".into(),
            description: None,
            capabilities: vec![Capability::ReadPages, Capability::QueryIndex],
            entry: None,
            disabled: false,
        };
        assert!(m.validate().is_ok());
    }

    #[test]
    fn manifest_validate_empty_is_ok() {
        let m = PluginManifest {
            id: "test".into(),
            name: "Test".into(),
            version: "1.0".into(),
            description: None,
            capabilities: vec![],
            entry: None,
            disabled: false,
        };
        assert!(m.validate().is_ok());
    }

    #[test]
    fn scan_plugins_rejects_unknown_capability() {
        let temp = tempfile::tempdir().unwrap();
        let plugins_dir = temp.path().join("app/plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("bad.toml"), r#"
id = "bad-plugin"
name = "Bad Plugin"
version = "1.0"
capabilities = ["read-pages", "do-anything"]
"#).unwrap();
        let registry = scan_plugins(temp.path()).unwrap();
        // bad-plugin should be skipped because "do-anything" is unknown
        assert!(registry.plugins.is_empty(), "plugin with unknown capability should be skipped");
    }

    #[test]
    fn scan_plugins_accepts_valid_caps() {
        let temp = tempfile::tempdir().unwrap();
        let plugins_dir = temp.path().join("app/plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("good.toml"), r#"
id = "good-plugin"
name = "Good Plugin"
version = "1.0"
capabilities = ["read-pages", "query-index"]
"#).unwrap();
        let registry = scan_plugins(temp.path()).unwrap();
        assert_eq!(registry.plugins.len(), 1);
        assert_eq!(registry.plugins[0].id, "good-plugin");
    }
}
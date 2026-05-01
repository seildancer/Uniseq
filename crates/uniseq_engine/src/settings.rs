//! Application settings persisted under app/settings.toml.
//!
//! Extends WorkspaceConfig (under app/config.toml) with editor, theme,
//! calendar, sync, and plugin-related user preferences. Backward-compatible
//! with existing WorkspaceConfig — stored separately so config.toml stays
//! minimal.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Toml(String),
}

/// User-facing application settings. Stored in app/settings.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Editor preferences.
    pub editor: EditorSettings,
    /// Theme and UI preferences.
    pub theme: ThemeSettings,
    /// Calendar display preferences.
    pub calendar: CalendarSettings,
    /// Sync configuration (local-first, no remote required).
    pub sync: SyncSettings,
    /// Plugin management settings.
    pub plugins: PluginSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            editor: EditorSettings::default(),
            theme: ThemeSettings::default(),
            calendar: CalendarSettings::default(),
            sync: SyncSettings::default(),
            plugins: PluginSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    /// Auto-save interval in seconds (0 = disabled).
    pub auto_save_seconds: u32,
    /// Default file format extension.
    pub default_extension: String,
    /// Enable spell check.
    pub spell_check: bool,
    /// Indent size.
    pub indent_size: u8,
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings {
            auto_save_seconds: 30,
            default_extension: "md".into(),
            spell_check: true,
            indent_size: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSettings {
    /// Light, dark, or system.
    pub mode: String,
    /// Accent color as hex string.
    pub accent: String,
    /// Font size in pixels.
    pub font_size: u16,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        ThemeSettings {
            mode: "system".into(),
            accent: "#4f46e5".into(),
            font_size: 14,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarSettings {
    /// First day of week: 0=Sun, 1=Mon.
    pub week_start: u8,
    /// Preferred date format string.
    pub date_format: String,
    /// Show week numbers.
    pub show_week_numbers: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        CalendarSettings {
            week_start: 1,
            date_format: "%Y-%m-%d".into(),
            show_week_numbers: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSettings {
    /// Enable local-first sync manifest tracking.
    pub enabled: bool,
    /// Directory where sync manifests are stored (relative to workspace root).
    pub manifest_dir: String,
    /// Last known sync timestamp (unix ms).
    pub last_sync_ms: Option<i64>,
}

impl Default for SyncSettings {
    fn default() -> Self {
        SyncSettings {
            enabled: false,
            manifest_dir: ".uniseq/sync".into(),
            last_sync_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettings {
    /// Enable plugin system.
    pub enabled: bool,
    /// Directories to scan for plugins.
    pub plugin_dirs: Vec<String>,
    /// IDs of explicitly disabled plugins.
    pub disabled_plugins: Vec<String>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        PluginSettings {
            enabled: false,
            plugin_dirs: vec!["app/plugins".into(), "plugins".into()],
            disabled_plugins: Vec::new(),
        }
    }
}

/// Load app settings (returns default if absent or corrupt).
pub fn load_settings(root: impl AsRef<Path>) -> Result<AppSettings, SettingsError> {
    let path = root.as_ref().join("app").join("settings.toml");
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| SettingsError::Toml(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(e) => Err(e.into()),
    }
}

/// Save app settings.
pub fn save_settings(root: impl AsRef<Path>, settings: &AppSettings) -> Result<(), SettingsError> {
    let path = root.as_ref().join("app").join("settings.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(settings).map_err(|e| SettingsError::Toml(e.to_string()))?;
    fs::write(&path, text)?;
    Ok(())
}
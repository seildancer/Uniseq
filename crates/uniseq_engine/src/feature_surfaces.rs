//! Feature surface metadata: available / deferred status for each major UI surface.
//!
//! Provides enough data for UI to display availability indicators and storage
//! directories without building real functionality.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// State of a feature surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeatureStatus {
    /// Available and ready to use.
    Available,
    /// Declined or turned off by configuration.
    Disabled,
    /// Not yet implemented or detected.
    Deferred,
}

/// A single feature surface with its status and storage path hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSurface {
    /// Unique surface identifier.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Whether this surface is currently available.
    pub status: FeatureStatus,
    /// Optional root directory where this surface stores data (relative to workspace).
    pub storage_dir: Option<String>,
    /// Optional description.
    pub note: Option<String>,
}

/// All feature surfaces in the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureRegistry {
    pub surfaces: Vec<FeatureSurface>,
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        FeatureRegistry {
            surfaces: vec![
                FeatureSurface {
                    id: "pdf".into(),
                    name: "PDF Viewer".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: Some("pdf".into()),
                    note: Some("PDF files stored in the pdf/ directory".into()),
                },
                FeatureSurface {
                    id: "whiteboard".into(),
                    name: "Whiteboard".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: Some("whiteboards".into()),
                    note: Some("Whiteboard assets stored in whiteboards/".into()),
                },
                FeatureSurface {
                    id: "flashcards".into(),
                    name: "Flashcards".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: Some("app/flashcards".into()),
                    note: Some("Flashcard data stored under app/flashcards".into()),
                },
                FeatureSurface {
                    id: "browser".into(),
                    name: "Browser / Webview".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: None,
                    note: Some("Browser surface is not yet integrated".into()),
                },
                FeatureSurface {
                    id: "mobile".into(),
                    name: "Mobile Interface".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: None,
                    note: Some("Mobile interface is out-of-scope for this engine version".into()),
                },
                FeatureSurface {
                    id: "graph".into(),
                    name: "Graph View".into(),
                    status: FeatureStatus::Available,
                    storage_dir: None,
                    note: Some("Graph data derived from workspace index".into()),
                },
                FeatureSurface {
                    id: "assets".into(),
                    name: "Assets".into(),
                    status: FeatureStatus::Available,
                    storage_dir: Some("assets".into()),
                    note: Some("Asset registry is derived from files and markdown references".into()),
                },
                FeatureSurface {
                    id: "sync".into(),
                    name: "Sync Status".into(),
                    status: FeatureStatus::Deferred,
                    storage_dir: Some(".uniseq/sync".into()),
                    note: Some("Local manifest scanner exists; remote sync is deferred".into()),
                },
                FeatureSurface {
                    id: "plugin".into(),
                    name: "Plugin System".into(),
                    status: FeatureStatus::Disabled,
                    storage_dir: Some("app/plugins".into()),
                    note: Some("Plugin system present but disabled by default".into()),
                },
            ],
        }
    }
}

/// Update surface status based on workspace configuration or detected files.
pub fn update_surface_statuses(root: &PathBuf) -> FeatureRegistry {
    let mut reg = FeatureRegistry::default();
    for surf in &mut reg.surfaces {
        match surf.id.as_str() {
            // PDF/whiteboard/flashcards — not yet implemented, always deferred
            "pdf" | "whiteboard" | "flashcards" => {
                surf.status = FeatureStatus::Deferred;
            }
            // Browser/mobile — no engine integration, always deferred
            "browser" | "mobile" => {
                surf.status = FeatureStatus::Deferred;
            }
            "assets" => {
                surf.status = FeatureStatus::Available;
            }
            // Sync has a local manifest scanner only; remote protocol remains deferred.
            "sync" => {
                surf.status = FeatureStatus::Deferred;
            }
            // Plugin — only available if explicitly enabled by settings (deferred by default)
            "plugin" => {
                surf.status = FeatureStatus::Deferred;
            }
            // Graph — available if the workspace has any pages/journals (graph_data can be built)
            "graph" => {
                if root.join("pages").is_dir() || root.join("journals").is_dir() {
                    surf.status = FeatureStatus::Available;
                } else {
                    surf.status = FeatureStatus::Deferred;
                }
            }
            _ => {}
        }
    }
    reg
}

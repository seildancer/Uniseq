//! Serializable event payloads for the Uniseq engine.
//!
//! These DTOs are ready for Tauri emit and can be derived from write
//! invalidations or engine state transitions. No runtime event bus is
//! required; the structs here represent the *shape* of events.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level engine event envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub kind: EngineEventKind,
    pub workspace_root: PathBuf,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EngineEventKind {
    WorkspaceOpened { journal_count: usize, page_count: usize },
    FileChanged { path: PathBuf, kind: FileChangeKind },
    IndexRebuilt { entry_count: usize, duration_ms: u64 },
    PageInvalidated { page_path: String },
    SearchIndexUpdated { document_count: usize },
    SyncStatusChanged { status: SyncStatus },
    ConflictDetected { path: PathBuf, resolution: ConflictResolution },
}

/// Kind of file change that triggered an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

/// Local-first sync status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncStatus {
    Idle,
    Scanning,
    Syncing,
    Conflict,
    Error(String),
}

/// How a sync conflict was or will be resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution {
    Pending,
    KeptLocal,
    KeptRemote,
    Merged,
}

/// Build an EngineEvent from a workspace-open action.
pub fn workspace_opened_event(
    workspace_root: PathBuf,
    journal_count: usize,
    page_count: usize,
) -> EngineEvent {
    EngineEvent {
        kind: EngineEventKind::WorkspaceOpened { journal_count, page_count },
        workspace_root,
        timestamp_ms: now_ms(),
    }
}

/// Build a PageInvalidated event from a write result's invalidation list.
pub fn page_invalidated_from_invalidations(
    workspace_root: &PathBuf,
    invalidations: &[PathBuf],
) -> Vec<EngineEvent> {
    invalidations
        .iter()
        .filter_map(|p| {
            let components: Vec<_> = p.components().collect();
            // Look for pages/<name>.md or journals/<date>.md
            if let Some(pos) = components.iter().position(|c| c.as_os_str() == "pages") {
                if let Some(name) = components.get(pos + 1) {
                    let s = name.as_os_str().to_string_lossy();
                    if s.ends_with(".md") {
                        let page_path = s.trim_end_matches(".md")
                            .replace("___", "/");
                        return Some(EngineEvent {
                            kind: EngineEventKind::PageInvalidated { page_path },
                            workspace_root: workspace_root.clone(),
                            timestamp_ms: now_ms(),
                        });
                    }
                }
            }
            None
        })
        .collect()
}

/// Build file-changed events from a write result.
pub fn file_changed_events(
    workspace_root: PathBuf,
    invalidations: &[PathBuf],
) -> Vec<EngineEvent> {
    invalidations.iter().map(|p| EngineEvent {
        kind: EngineEventKind::FileChanged {
            path: p.clone(),
            kind: FileChangeKind::Modified,
        },
        workspace_root: workspace_root.clone(),
        timestamp_ms: now_ms(),
    }).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
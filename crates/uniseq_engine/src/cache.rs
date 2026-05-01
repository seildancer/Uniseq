//! Persisted disposable cache for index snapshots.
//!
//! Cache is a performance hint — the index is always rebuilt from source files
//! on cache miss or corruption. The cache is never the source of truth.

use crate::index::{build_index, WorkspaceIndex};
use crate::model::WorkspaceWarning;
use crate::workspace::{open_workspace, WorkspaceError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cache is corrupted or incompatible: {0}")]
    Corrupted(String),
}

/// Snapshot saved under .cache/index_snapshot_v{N}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSnapshot {
    pub version: u32,
    pub root: PathBuf,
    pub source_fingerprint: String,
    pub index: WorkspaceIndex,
    pub warnings: Vec<WorkspaceWarning>,
}

/// Save an index snapshot to the cache directory.
/// Overwrites any existing snapshot of the same version.
pub fn save_snapshot(
    root: impl AsRef<Path>,
    index: WorkspaceIndex,
    warnings: Vec<WorkspaceWarning>,
) -> Result<(), CacheError> {
    let cache_dir = root.as_ref().join(".cache");
    fs::create_dir_all(&cache_dir)?;
    let snapshot = IndexSnapshot {
        version: 1,
        root: root.as_ref().to_path_buf(),
        source_fingerprint: source_fingerprint(root.as_ref())?,
        index,
        warnings,
    };
    let path = cache_dir.join("index_snapshot_v1.json");
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| CacheError::Corrupted(e.to_string()))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, json.as_bytes())?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Load an index snapshot from the cache directory.
///
/// Returns `Ok(None)` if no cache file exists or is unreadable.
/// Returns `Err` only on unexpected corruption (not on cache miss).
pub fn load_snapshot(root: impl AsRef<Path>) -> Result<Option<IndexSnapshot>, CacheError> {
    let path = root.as_ref().join(".cache").join("index_snapshot_v1.json");
    match fs::read_to_string(&path) {
        Ok(text) => {
            let snapshot: IndexSnapshot =
                serde_json::from_str(&text).map_err(|e| CacheError::Corrupted(e.to_string()))?;
            if snapshot.version != 1
                || snapshot.root != root.as_ref()
                || snapshot.source_fingerprint != source_fingerprint(root.as_ref())?
            {
                return Ok(None);
            }
            Ok(Some(snapshot))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Load snapshot, falling back to a full index rebuild on cache miss or corruption.
/// The returned warnings reflect the state *after* rebuild.
pub fn load_or_rebuild(
    root: impl AsRef<Path>,
) -> Result<(WorkspaceIndex, Vec<WorkspaceWarning>), CacheError> {
    match load_snapshot(root.as_ref()) {
        Ok(Some(snapshot)) => Ok((snapshot.index, snapshot.warnings)),
        // cache miss or corruption — fall through to rebuild
        Ok(None) | Err(_) => {
            let summary = open_workspace(root.as_ref()).map_err(CacheError::Workspace)?;
            let index =
                build_index(root.as_ref()).map_err(|e| CacheError::Corrupted(e.to_string()))?;
            let warnings = summary.warnings;
            // Best-effort cache write; failure is non-fatal
            let _ = save_snapshot(root, index.clone(), warnings.clone());
            Ok((index, warnings))
        }
    }
}

/// Remove any cached snapshot (for forcing a full rebuild).
pub fn clear_snapshot(root: impl AsRef<Path>) -> Result<(), CacheError> {
    let path = root.as_ref().join(".cache").join("index_snapshot_v1.json");
    fs::remove_file(path).ok(); // ignore NotFound
    Ok(())
}

fn source_fingerprint(root: &Path) -> Result<String, CacheError> {
    let mut parts = Vec::new();
    for dir in [root.join("journals"), root.join("pages")] {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        {
            let metadata = fs::metadata(entry.path())?;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .display()
                .to_string();
            parts.push(format!("{rel}:{}:{modified}", metadata.len()));
        }
    }
    parts.sort();
    Ok(parts.join("|"))
}

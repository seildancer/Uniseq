//! Local-first sync data structures and scan stub.
//!
//! No real remote networking required. Provides deterministic sync status
//! based on file/manifest inspection.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("walkdir error: {0}")]
    WalkDir(#[from] walkdir::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
}

/// A convenience alias for the main sync result type.
pub type SyncResult<T> = Result<T, SyncError>;

/// A tracked file entry in the local sync manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifestEntry {
    pub path: PathBuf,
    /// Last known content hash (arbitrary string, e.g. MD5 prefix).
    pub content_hash: String,
    /// Last modified timestamp of file at time of manifest write.
    pub modified_ms: u64,
    /// Sequence number for ordering.
    pub seq: u64,
}

/// The local sync manifest.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncManifest {
    pub version: u32,
    pub workspace_root: PathBuf,
    pub entries: Vec<SyncManifestEntry>,
    pub seq: u64,
}

/// Current sync state for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub status: SyncStatus,
    pub manifest_path: PathBuf,
    pub local_seq: u64,
    pub conflicts: Vec<ConflictEntry>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConflictEntry {
    pub path: PathBuf,
    pub local_hash: String,
    pub remote_hash: Option<String>,
    pub detected_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncStatus {
    Idle,
    Scanning,
    Syncing,
    Conflict,
    Error(String),
}

impl Default for SyncStatus {
    fn default() -> Self { SyncStatus::Idle }
}

/// Load existing manifest or return empty default.
pub fn load_manifest(root: impl AsRef<Path>) -> SyncManifest {
    let path = root.as_ref().join(".uniseq").join("sync").join("manifest.toml");
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        _ => SyncManifest::default(),
    }
}

/// Save manifest atomically.
pub fn save_manifest(root: impl AsRef<Path>, manifest: &SyncManifest) -> Result<(), SyncError> {
    let base = root.as_ref().join(".uniseq").join("sync");
    fs::create_dir_all(&base)?;
    let path = base.join("manifest.toml");
    let text = toml::to_string_pretty(manifest).map_err(|e| SyncError::Manifest(e.to_string()))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Compute deterministic content hash from file bytes (first 8KB + size).
/// This is a stub — deterministic based on content, not time.
pub fn content_hash(path: &Path) -> Result<String, SyncError> {
    let bytes = fs::read(path)?;
    let size = bytes.len() as u64;
    // Simple stub: size + first 64 bytes hex
    let prefix = bytes.iter().take(64).map(|b| format!("{:02x}", b)).collect::<String>();
    Ok(format!("{}:{}", size, prefix))
}

/// Scan the workspace and build a new manifest. Returns (manifest, conflicts).
pub fn build_manifest(root: impl AsRef<Path>) -> Result<(SyncManifest, Vec<ConflictEntry>), SyncError> {
    let root = root.as_ref();
    let mut entries = Vec::new();
    let mut conflicts = Vec::new();
    let mut seq = 0u64;

    let tracked_dirs = ["journals", "pages", "assets"];
    for dir in tracked_dirs {
        let dir_path = root.join(dir);
        if !dir_path.is_dir() { continue; }
        for entry in walkdir::WalkDir::new(&dir_path).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() { continue; }
            if entry.path().extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            seq += 1;
            let hash = content_hash(entry.path())?;
            let modified_ms = entry.metadata()?.modified()?
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
            entries.push(SyncManifestEntry {
                path: entry.path().strip_prefix(root).unwrap_or(entry.path()).to_path_buf(),
                content_hash: hash,
                modified_ms,
                seq,
            });
        }
    }

    let manifest = SyncManifest {
        version: 1,
        workspace_root: root.to_path_buf(),
        entries,
        seq,
    };

    // Detect conflicts: files whose current hash differs from manifest's last hash
    let old = load_manifest(root);
    for new_entry in &manifest.entries {
        if let Some(old_entry) = old.entries.iter().find(|e| e.path == new_entry.path) {
            if old_entry.content_hash != new_entry.content_hash && old_entry.seq > manifest.seq.saturating_sub(10) {
                // Suspicious change: the file changed but we didn't update manifest
                // (Could be concurrent external edit — mark as conflict)
                conflicts.push(ConflictEntry {
                    path: new_entry.path.clone(),
                    local_hash: new_entry.content_hash.clone(),
                    remote_hash: Some(old_entry.content_hash.clone()),
                    detected_at_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64,
                });
            }
        }
    }

    Ok((manifest, conflicts))
}

/// Stub: compute sync plan — returns current state and what would be synced.
/// No actual remote operations.
pub fn sync_plan(root: impl AsRef<Path>) -> Result<SyncState, SyncError> {
    let root = root.as_ref();
    let (manifest, conflicts) = build_manifest(root)?;
    let state = SyncState {
        status: if conflicts.is_empty() { SyncStatus::Idle } else { SyncStatus::Conflict },
        manifest_path: root.join(".uniseq").join("sync").join("manifest.toml"),
        local_seq: manifest.seq,
        conflicts,
        errors: Vec::new(),
    };
    Ok(state)
}

/// Stub: perform a sync scan (no remote). Saves manifest and returns final state.
pub fn sync_now(root: impl AsRef<Path>) -> Result<SyncState, SyncError> {
    let root_ref = root.as_ref();
    let (manifest, conflicts) = build_manifest(root_ref)?;
    save_manifest(root_ref, &manifest)?;
    Ok(SyncState {
        status: SyncStatus::Idle,
        manifest_path: root_ref.join(".uniseq").join("sync").join("manifest.toml"),
        local_seq: manifest.seq,
        conflicts,
        errors: Vec::new(),
    })
}
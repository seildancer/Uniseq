//! Asset registry: scans assets/ and markdown references to build an asset index.
//!
//! Extracts Rust-owned markdown asset references from journal/page entries,
//! recognizing: assets/foo.png, ![](assets/foo.png), ![alt](assets/foo.png),
//! [text](assets/foo.png), and bare relative paths under assets/.

use crate::model::{PageRef, SourceAnchor, Span};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A registered asset with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    /// Relative path from workspace root (forward slashes).
    pub relative_path: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Last-modified unix timestamp-ms.
    pub modified_ms: u64,
    /// Page paths that reference this asset.
    pub referenced_by: Vec<ReferencedAnchor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencedAnchor {
    pub page_path: String,
    pub anchor: SourceAnchor,
}

/// Registry of all assets in a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetRegistry {
    pub assets: Vec<AssetRecord>,
}

/// Extract asset relative paths from all entries in a workspace index.
/// This is the Rust-owned extraction: walks journals/pages markdown content
/// to find asset references not limited to page links.
pub fn extract_asset_refs_from_entries(
    root: impl AsRef<Path>,
) -> Vec<(String, Vec<AssetRefMatch>)> {
    let mut result: Vec<(String, Vec<AssetRefMatch>)> = Vec::new();
    let root = root.as_ref();
    let journal_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();

    for dir in [root.join("journals"), root.join("pages")] {
        if !dir.is_dir() { continue; }
        for entry in WalkDir::new(&dir).into_iter().filter_map(Result::ok) {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let text = match std::fs::read_to_string(entry.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let source_file = entry.path().to_string_lossy().to_string();
            let mut refs = extract_asset_refs_from_text(&text);

            for cap in journal_re.captures_iter(&text) {
                if let Some(m) = cap.get(1) {
                    let raw = m.as_str().trim();
                    if let Some(rel_path) = extract_asset_rel(raw) {
                        let whole = cap.get(0).unwrap();
                        refs.push(AssetRefMatch {
                            raw: whole.as_str().to_string(),
                            path_text: raw.to_string(),
                            rel_path,
                            span: Span { start: whole.start(), end: whole.end() },
                            path_span: Span { start: m.start(), end: m.end() },
                        });
                    }
                }
            }

            if !refs.is_empty() { result.push((source_file, refs)); }
        }
    }
    result
}

#[derive(Debug, Clone)]
pub struct AssetRefMatch {
    pub raw: String,
    pub path_text: String,
    pub rel_path: String,
    pub span: Span,
    pub path_span: Span,
}

pub fn extract_asset_refs_from_text(text: &str) -> Vec<AssetRefMatch> {
    let mut refs = Vec::new();
    let md_link_re = Regex::new(r#"!?\[[^\]]*\]\(([^)\s]+)\)"#).unwrap();
    for cap in md_link_re.captures_iter(text) {
        let Some(url) = cap.get(1) else { continue; };
        if let Some(rel_path) = extract_asset_rel(url.as_str()) {
            let whole = cap.get(0).unwrap();
            refs.push(AssetRefMatch {
                raw: whole.as_str().to_string(),
                path_text: url.as_str().to_string(),
                rel_path,
                span: Span { start: whole.start(), end: whole.end() },
                path_span: Span { start: url.start(), end: url.end() },
            });
        }
    }

    let plain_re = Regex::new(r#"(?:^|\s)(\.?/?assets/[^\s\]")]+)"#).unwrap();
    for cap in plain_re.captures_iter(text) {
        let Some(m) = cap.get(1) else { continue; };
        let path_text = m.as_str().trim_end_matches(')').to_string();
        if let Some(rel_path) = extract_asset_rel(&path_text) {
            refs.push(AssetRefMatch {
                raw: path_text.clone(),
                path_text: path_text.clone(),
                rel_path,
                span: Span { start: m.start(), end: m.start() + path_text.len() },
                path_span: Span { start: m.start(), end: m.start() + path_text.len() },
            });
        }
    }
    refs
}

pub fn scan_assets(root: impl AsRef<Path>, _index_refs: &[(String, Vec<PageRef>)]) -> AssetRegistry {
    let root = root.as_ref();
    let assets_dir = root.join("assets");
    let mut records: Vec<AssetRecord> = Vec::new();
    if assets_dir.is_dir() {
        for entry in WalkDir::new(&assets_dir).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(&assets_dir).unwrap_or(entry.path()).to_string_lossy().replace('\\', "/");
                let meta = entry.metadata().ok();
                let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified_ms = meta.and_then(|m| m.modified().ok()).map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64).unwrap_or(0);
                records.push(AssetRecord { relative_path: rel, size_bytes, modified_ms, referenced_by: Vec::new() });
            }
        }
    }
    let asset_refs = extract_asset_refs_from_entries(root);
    for (source_file, refs) in asset_refs {
        for r in refs {
            if let Some(rec) = records.iter_mut().find(|rec| rec.relative_path == r.rel_path) {
                rec.referenced_by.push(ReferencedAnchor { page_path: source_file.clone(), anchor: SourceAnchor { file_path: PathBuf::from(&source_file), span: r.span, snippet: r.raw.clone() } });
            }
        }
    }
    records.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    AssetRegistry { assets: records }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetQuery { pub prefix: Option<String>, pub min_size: Option<u64>, pub max_size: Option<u64>, pub referenced: Option<bool> }

pub fn query_assets(registry: &AssetRegistry, q: &AssetQuery) -> Vec<AssetRecord> {
    registry.assets.iter().filter(|a| {
        if let Some(ref p) = q.prefix { if !a.relative_path.starts_with(p) { return false; } }
        if let Some(min) = q.min_size { if a.size_bytes < min { return false; } }
        if let Some(max) = q.max_size { if a.size_bytes > max { return false; } }
        if let Some(r) = q.referenced { if (!a.referenced_by.is_empty()) != r { return false; } }
        true
    }).cloned().collect()
}

pub fn extract_asset_rel(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim_end_matches(')');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") { return None; }
    if trimmed.starts_with("assets/") { return Some(trimmed.trim_start_matches("assets/").to_string()); }
    if trimmed.starts_with("./assets/") { return Some(trimmed.trim_start_matches("./assets/").to_string()); }
    if trimmed.starts_with("/assets/") { return Some(trimmed.trim_start_matches("/assets/").to_string()); }
    if trimmed.contains("assets/") && !trimmed.starts_with("http") {
        let idx = trimmed.find("assets/").unwrap();
        return Some(trimmed[idx + "assets/".len()..].to_string());
    }
    None
}

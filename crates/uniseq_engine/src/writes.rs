use crate::model::{SourceAnchor, Span, TaskState};
use crate::page_identity::{normalize_page_path, page_path_to_filename};
use crate::assets::extract_asset_refs_from_text;
use chrono::NaiveDate;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("source anchor is stale for {path:?}")]
    StaleAnchor { path: PathBuf },
    #[error("invalid task marker at requested anchor")]
    InvalidTaskMarker,
    #[error("asset path must stay inside the workspace assets directory")]
    InvalidAssetPath,
    #[error("target page already exists: {0}")]
    TargetPageExists(PathBuf),
    #[error("page name must normalize to a non-empty workspace page path")]
    InvalidPageName,
    #[error("workspace content directory escapes the workspace root")]
    InvalidWorkspacePath,
}

/// Result of a write operation: the new anchor and any file paths that were
/// modified (including renames). Callers can use invalidated to keep caches fresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResult {
    /// The new anchor after the write (for anchor-preserving operations).
    pub anchor: Option<SourceAnchor>,
    /// All file paths that were modified by this operation.
    pub invalidated: Vec<PathBuf>,
}

impl WriteResult {
    pub fn anchor_only(anchor: SourceAnchor) -> Self {
        let file_path = anchor.file_path.clone();
        WriteResult {
            anchor: Some(anchor),
            invalidated: vec![file_path],
        }
    }
    pub fn unit(invalidated: Vec<PathBuf>) -> Self {
        WriteResult {
            anchor: None,
            invalidated,
        }
    }
}

/// Read the current task state from a file anchor without modifying anything.
/// Returns `None` if the anchor is not on a task line.
pub fn read_task_state(anchor: &SourceAnchor) -> Result<Option<TaskState>, WriteError> {
    let text = fs::read_to_string(&anchor.file_path)?;
    validate_anchor(&text, anchor)?;
    let line = text
        .get(anchor.span.start..anchor.span.end)
        .ok_or_else(|| WriteError::StaleAnchor {
            path: anchor.file_path.clone(),
        })?;
    let marker_pos = line.find("- [").ok_or(WriteError::InvalidTaskMarker)?;
    let absolute = anchor.span.start + marker_pos;
    let marker = text
        .get(absolute..absolute + 5)
        .ok_or(WriteError::InvalidTaskMarker)?;
    let state = match marker {
        "- [ ]" => Some(TaskState::Todo),
        "- [x]" | "- [X]" => Some(TaskState::Done),
        _ => None,
    };
    Ok(state)
}

pub fn append_journal_entry(
    root: impl AsRef<Path>,
    date: NaiveDate,
    markdown: &str,
) -> Result<WriteResult, WriteError> {
    let path = root.as_ref().join("journals").join(format!("{date}.md"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut existing = fs::read_to_string(&path).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    let start = existing.len();
    let mut addition = markdown.trim_end().to_string();
    addition.push('\n');
    existing.push_str(&addition);
    atomic_write(&path, existing.as_bytes())?;
    let snippet = addition.trim_end().to_string();
    Ok(WriteResult::anchor_only(SourceAnchor {
        file_path: path,
        span: Span {
            start,
            end: start + snippet.len(),
        },
        snippet,
    }))
}

pub fn toggle_task(
    anchor: &SourceAnchor,
    desired: Option<TaskState>,
) -> Result<WriteResult, WriteError> {
    let mut text = fs::read_to_string(&anchor.file_path)?;
    validate_anchor(&text, anchor)?;
    let line = text
        .get(anchor.span.start..anchor.span.end)
        .ok_or_else(|| WriteError::StaleAnchor {
            path: anchor.file_path.clone(),
        })?;
    let marker_pos = line.find("- [").ok_or(WriteError::InvalidTaskMarker)?;
    let absolute = anchor.span.start + marker_pos;
    let Some(marker) = text.get(absolute..absolute + 5) else {
        return Err(WriteError::InvalidTaskMarker);
    };
    let current = match marker {
        "- [ ]" => TaskState::Todo,
        "- [x]" | "- [X]" => TaskState::Done,
        _ => return Err(WriteError::InvalidTaskMarker),
    };
    let next = desired.unwrap_or(match current {
        TaskState::Todo => TaskState::Done,
        TaskState::Done => TaskState::Todo,
    });
    let replacement = match next {
        TaskState::Todo => "- [ ]",
        TaskState::Done => "- [x]",
    };
    if text.get(absolute..absolute + 5).is_none() {
        return Err(WriteError::InvalidTaskMarker);
    }
    text.replace_range(absolute..absolute + 5, replacement);
    atomic_write(&anchor.file_path, text.as_bytes())?;
    let snippet = text
        .get(anchor.span.start..anchor.span.end)
        .ok_or_else(|| WriteError::StaleAnchor {
            path: anchor.file_path.clone(),
        })?
        .to_string();
    Ok(WriteResult::anchor_only(SourceAnchor {
        file_path: anchor.file_path.clone(),
        span: anchor.span,
        snippet,
    }))
}

pub fn edit_markdown_span(
    anchor: &SourceAnchor,
    replacement: &str,
) -> Result<WriteResult, WriteError> {
    let mut text = fs::read_to_string(&anchor.file_path)?;
    validate_anchor(&text, anchor)?;
    text.replace_range(anchor.span.start..anchor.span.end, replacement);
    atomic_write(&anchor.file_path, text.as_bytes())?;
    let new_anchor = SourceAnchor {
        file_path: anchor.file_path.clone(),
        span: Span {
            start: anchor.span.start,
            end: anchor.span.start + replacement.len(),
        },
        snippet: replacement.to_string(),
    };
    Ok(WriteResult::anchor_only(new_anchor))
}

pub fn update_page_front_matter(
    root: impl AsRef<Path>,
    page_path: &str,
    front_matter_body: &str,
) -> Result<WriteResult, WriteError> {
    let path = root
        .as_ref()
        .join("pages")
        .join(page_path_to_filename(page_path));
    let text = fs::read_to_string(&path).unwrap_or_default();
    let new_front_matter = format!("---\n{}\n---\n", front_matter_body.trim());
    let updated = if text.starts_with("---\n") || text.starts_with("---\r\n") {
        let mut offset = if text.starts_with("---\r\n") { 5 } else { 4 };
        let mut found = None;
        for line in text[offset..].split_inclusive('\n') {
            if line.trim() == "---" {
                found = Some(offset + line.len());
                break;
            }
            offset += line.len();
        }
        match found {
            Some(end) => format!("{}{}", new_front_matter, &text[end..]),
            None => format!("{}{}", new_front_matter, text),
        }
    } else {
        format!("{}{}", new_front_matter, text)
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(&path, updated.as_bytes())?;
    Ok(WriteResult::unit(vec![path]))
}

pub fn rename_page(
    root: impl AsRef<Path>,
    from: &str,
    to: &str,
) -> Result<WriteResult, WriteError> {
    let root = root.as_ref();
    let from_norm = normalize_page_path(from);
    let to_norm = normalize_page_path(to);
    if from_norm.is_empty() || to_norm.is_empty() {
        return Err(WriteError::InvalidPageName);
    }
    let root = root.canonicalize().map_err(|_| WriteError::InvalidPageName)?;
    let pages_dir = canonical_workspace_child(&root, "pages", true)?;
    let journals_dir = canonical_workspace_child(&root, "journals", false).ok();
    let from_file = pages_dir.join(page_path_to_filename(&from_norm));
    let to_file = pages_dir.join(page_path_to_filename(&to_norm));
    if from_file != to_file && to_file.exists() {
        return Err(WriteError::TargetPageExists(to_file));
    }
    let link_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    let tag_re = Regex::new(&format!(r"(?i)(^|\s)#{}\b", regex::escape(&from_norm))).unwrap();
    let mut patches: Vec<(PathBuf, String)> = Vec::new();
    for dir in [journals_dir, Some(pages_dir.clone())].into_iter().flatten() {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        {
            let path = entry.path();
            if path == from_file {
                continue;
            }
            let text = fs::read_to_string(path)?;
            let after_norm_link = replace_wiki_links(&link_re, &text, &from_norm, to);
            let replaced_tags = tag_re.replace_all(&after_norm_link, format!("$1#{to_norm}"));
            if replaced_tags != text {
                patches.push((path.to_path_buf(), replaced_tags.into_owned()));
            }
        }
    }

    let backups = patches
        .iter()
        .map(|(path, _)| Ok((path.clone(), fs::read_to_string(path)?)))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    if let Some(parent) = to_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let renamed = if from_file.exists() {
        fs::rename(&from_file, &to_file)?;
        true
    } else {
        false
    };
    let mut invalidated = vec![to_file.clone()];
    if renamed {
        invalidated.push(from_file.clone());
    }
    for (path, replacement) in patches {
        if let Err(err) = atomic_write(&path, replacement.as_bytes()) {
            for (backup_path, original) in &backups {
                let _ = fs::write(backup_path, original);
            }
            if renamed {
                let _ = fs::rename(&to_file, &from_file);
            }
            return Err(err);
        }
        let p = path.to_path_buf();
        if !invalidated.contains(&p) {
            invalidated.push(p);
        }
    }
    // Handle self-referential links within the renamed page file itself.
    // After the file has been moved, any [[old-name]] or #old-name self-references
    // in the new file must be updated to reflect the new name.
    if renamed {
        if let Ok(text) = fs::read_to_string(&to_file) {
            let replaced_links = replace_wiki_links(&link_re, &text, &from_norm, to);
            let replaced_all = tag_re.replace_all(&replaced_links, format!("$1#{to_norm}"));
            if replaced_all != text {
                atomic_write(&to_file, replaced_all.as_bytes())?;
            }
        }
    }
    Ok(WriteResult::unit(invalidated))
}

pub fn move_asset(
    root: impl AsRef<Path>,
    from_relative: &str,
    to_relative: &str,
) -> Result<WriteResult, WriteError> {
    if !is_safe_relative(from_relative) || !is_safe_relative(to_relative) {
        return Err(WriteError::InvalidAssetPath);
    }
    let root = root.as_ref().canonicalize().map_err(|_| WriteError::InvalidAssetPath)?;
    let assets = root.join("assets");

    // Canonicalize assets directory once to enforce prefix boundary.
    let assets_canonical = assets.canonicalize().map_err(|_| WriteError::InvalidAssetPath)?;

    let from_path = assets.join(from_relative);
    let to_path = assets.join(to_relative);

    // Resolve symlinks/junctions on the source (which exists) and verify it stays within assets.
    let from_resolved = from_path.canonicalize().map_err(|_| WriteError::InvalidAssetPath)?;
    if !from_resolved.starts_with(&assets_canonical) {
        return Err(WriteError::InvalidAssetPath);
    }

    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)?;
        let parent_resolved = parent.canonicalize().map_err(|_| WriteError::InvalidAssetPath)?;
        if !parent_resolved.starts_with(&assets_canonical) {
            return Err(WriteError::InvalidAssetPath);
        }
    } else {
        return Err(WriteError::InvalidAssetPath);
    }

    let mut patches: Vec<(PathBuf, String)> = Vec::new();
    for dir in [root.join("journals"), root.join("pages")] {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        {
            let path = entry.path();
            let text = fs::read_to_string(path)?;
            let replaced = replace_asset_refs(&text, from_relative, to_relative);
            if replaced != text {
                patches.push((path.to_path_buf(), replaced));
            }
        }
    }
    let backups = patches
        .iter()
        .map(|(path, _)| Ok((path.clone(), fs::read_to_string(path)?)))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    fs::rename(&from_path, &to_path)?;
    let mut invalidated = vec![to_path.clone(), from_path.clone()];
    for (path, replacement) in patches {
        if let Err(err) = atomic_write(&path, replacement.as_bytes()) {
            for (backup_path, original) in &backups {
                let _ = fs::write(backup_path, original);
            }
            let _ = fs::rename(&to_path, &from_path);
            return Err(err);
        }
        if !invalidated.contains(&path) {
            invalidated.push(path);
        }
    }
    Ok(WriteResult::unit(invalidated))
}

fn replace_wiki_links(link_re: &Regex, text: &str, from_norm: &str, to: &str) -> String {
    link_re
        .replace_all(text, |caps: &Captures| {
            let raw_target = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            if normalize_page_path(raw_target) == from_norm {
                format!("[[{to}]]")
            } else {
                caps.get(0).map(|m| m.as_str()).unwrap_or_default().to_string()
            }
        })
        .into_owned()
}

fn replace_asset_refs(text: &str, from_relative: &str, to_relative: &str) -> String {
    let mut refs = extract_asset_refs_from_text(text)
        .into_iter()
        .filter(|r| r.rel_path == from_relative)
        .collect::<Vec<_>>();
    if refs.is_empty() {
        return text.to_string();
    }
    refs.sort_by_key(|r| r.path_span.start);
    let mut updated = text.to_string();
    for r in refs.into_iter().rev() {
        let replacement = if r.path_text.starts_with("./assets/") {
            format!("./assets/{to_relative}")
        } else if r.path_text.starts_with("/assets/") {
            format!("/assets/{to_relative}")
        } else if r.path_text.starts_with("assets/") {
            format!("assets/{to_relative}")
        } else {
            to_relative.to_string()
        };
        updated.replace_range(r.path_span.start..r.path_span.end, &replacement);
    }
    updated
}

fn canonical_workspace_child(root: &Path, name: &str, create: bool) -> Result<PathBuf, WriteError> {
    let dir = root.join(name);
    if create {
        fs::create_dir_all(&dir)?;
    }
    let canonical = dir.canonicalize()?;
    if !canonical.starts_with(root) {
        return Err(WriteError::InvalidWorkspacePath);
    }
    Ok(canonical)
}

fn validate_anchor(text: &str, anchor: &SourceAnchor) -> Result<(), WriteError> {
    if anchor.span.start > anchor.span.end
        || text.get(anchor.span.start..anchor.span.end) != Some(anchor.snippet.as_str())
    {
        return Err(WriteError::StaleAnchor {
            path: anchor.file_path.clone(),
        });
    }
    Ok(())
}

fn is_safe_relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

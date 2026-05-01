use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{
    DocumentKind, FrontMatter, FrontMatterPatch, FrontMatterValue, JournalDate, PageKey,
    ParsedDocument, ReferenceKind, SourceAnchor, Span, Task, page_file_name_for_key,
};
use crate::parser::parse_document;
use crate::workspace::Workspace;

#[derive(Debug)]
pub enum WriteError {
    Io(io::Error),
    StaleAnchor(String),
    Conflict(String),
    InvalidInput(String),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::StaleAnchor(message) => f.write_str(message),
            Self::Conflict(message) => f.write_str(message),
            Self::InvalidInput(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn append_journal_entry(
    workspace: &Workspace,
    date: &JournalDate,
    markdown: &str,
) -> Result<PathBuf, WriteError> {
    let path = workspace.paths.journals.join(format!("{date}.md"));
    fs::create_dir_all(&workspace.paths.journals)?;
    let existing = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let mut next = existing;
    if !next.is_empty() && !next.ends_with("\n\n") {
        if next.ends_with('\n') {
            next.push('\n');
        } else {
            next.push_str("\n\n");
        }
    }
    next.push_str(markdown.trim_end());
    next.push('\n');
    atomic_write(&path, &next)?;
    Ok(path)
}

pub fn edit_markdown_span(
    anchor: &SourceAnchor,
    expected_snippet: &str,
    replacement: &str,
) -> Result<(), WriteError> {
    let content = fs::read_to_string(&anchor.file_path)?;
    let span = resolve_anchor_span(&content, anchor, expected_snippet)?;
    let updated = replace_span(&content, &span, replacement);
    atomic_write(&anchor.file_path, &updated)?;
    Ok(())
}

pub fn toggle_task(task: &Task, checked: bool) -> Result<(), WriteError> {
    let content = fs::read_to_string(&task.anchor.file_path)?;
    let marker = if checked { "[x]" } else { "[ ]" };
    let span = resolve_span(
        &content,
        &task.marker_span,
        if task.checked { "[x]" } else { "[ ]" },
    )?;
    let updated = replace_span(&content, &span, marker);
    atomic_write(&task.anchor.file_path, &updated)?;
    Ok(())
}

pub fn update_page_front_matter(
    workspace: &Workspace,
    page_key: &PageKey,
    patch: &FrontMatterPatch,
) -> Result<PathBuf, WriteError> {
    let path = workspace
        .paths
        .pages
        .join(page_file_name_for_key(page_key));
    let content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let parsed = parse_document(
        DocumentKind::Page(page_key.clone()),
        path.strip_prefix(&workspace.paths.root)
            .unwrap_or(&path)
            .to_path_buf(),
        path.clone(),
        &content,
    )?;
    let mut front_matter = parsed.front_matter;
    for (key, value) in &patch.values {
        front_matter.values.insert(key.clone(), value.clone());
    }
    let serialized = serialize_front_matter(&front_matter, &parsed.body);
    atomic_write(&path, &serialized)?;
    Ok(path)
}

pub fn rename_page(
    workspace: &Workspace,
    old_key: &PageKey,
    new_key: &PageKey,
) -> Result<Vec<PathBuf>, WriteError> {
    if old_key == new_key {
        return Ok(Vec::new());
    }
    let mut file_changes: HashMap<PathBuf, String> = HashMap::new();
    let mut changed_paths = Vec::new();

    for absolute_path in workspace.workspace_markdown_files()? {
        let relative = absolute_path
            .strip_prefix(&workspace.paths.root)
            .unwrap_or(&absolute_path)
            .to_path_buf();
        let content = fs::read_to_string(&absolute_path)?;
        let kind = if absolute_path.starts_with(&workspace.paths.journals) {
            let file_name = relative.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
                WriteError::InvalidInput(format!("invalid journal path: {}", relative.display()))
            })?;
            DocumentKind::Journal(
                JournalDate::from_journal_file_name(file_name)
                    .ok_or_else(|| WriteError::InvalidInput("invalid journal file".to_string()))?,
            )
        } else {
            let page_file = relative.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
                WriteError::InvalidInput(format!("invalid page path: {}", relative.display()))
            })?;
            DocumentKind::Page(
                crate::model::page_key_from_page_file(page_file).ok_or_else(|| {
                    WriteError::InvalidInput(format!("unsupported page file: {}", relative.display()))
                })?,
            )
        };
        let parsed = parse_document(kind, relative.clone(), absolute_path.clone(), &content)?;
        let replacements = reference_replacements(&parsed, old_key, new_key);
        if replacements.is_empty() {
            continue;
        }
        file_changes.insert(absolute_path.clone(), apply_replacements(&content, &replacements));
        changed_paths.push(absolute_path);
    }

    let old_page_path = workspace
        .paths
        .pages
        .join(page_file_name_for_key(old_key));
    let new_page_path = workspace
        .paths
        .pages
        .join(page_file_name_for_key(new_key));
    if old_page_path.exists() && new_page_path.exists() {
        return Err(WriteError::Conflict(format!(
            "cannot rename {} because {} already exists",
            old_key, new_key
        )));
    }
    if old_page_path.exists() {
        let page_content = file_changes
            .remove(&old_page_path)
            .unwrap_or(fs::read_to_string(&old_page_path)?);
        atomic_write(&new_page_path, &page_content)?;
        fs::remove_file(&old_page_path)?;
        changed_paths.push(new_page_path);
        changed_paths.push(old_page_path);
    }

    let mut originals = BTreeMap::new();
    for (path, new_content) in &file_changes {
        originals.insert(path.clone(), fs::read_to_string(path)?);
        if let Err(error) = atomic_write(path, new_content) {
            for (restore_path, original) in originals {
                let _ = atomic_write(&restore_path, &original);
            }
            return Err(WriteError::Io(error));
        }
    }
    Ok(changed_paths)
}

pub fn move_asset(
    workspace: &Workspace,
    old_relative_asset_path: &Path,
    new_relative_asset_path: &Path,
) -> Result<Vec<PathBuf>, WriteError> {
    let old_path = workspace.paths.assets.join(old_relative_asset_path);
    let new_path = workspace.paths.assets.join(new_relative_asset_path);
    if !old_path.exists() {
        return Err(WriteError::InvalidInput(format!(
            "asset does not exist: {}",
            old_relative_asset_path.display()
        )));
    }
    if new_path.exists() {
        return Err(WriteError::Conflict(format!(
            "target asset already exists: {}",
            new_relative_asset_path.display()
        )));
    }

    let old_value = normalize_asset_reference(old_relative_asset_path);
    let new_value = normalize_asset_reference(new_relative_asset_path);
    let old_assets_value = format!("assets/{old_value}");
    let new_assets_value = format!("assets/{new_value}");
    let mut file_changes = HashMap::new();
    let mut changed = Vec::new();
    for absolute_path in workspace.workspace_markdown_files()? {
        let relative = absolute_path
            .strip_prefix(&workspace.paths.root)
            .unwrap_or(&absolute_path)
            .to_path_buf();
        let content = fs::read_to_string(&absolute_path)?;
        let kind = if absolute_path.starts_with(&workspace.paths.journals) {
            let file_name = relative.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
                WriteError::InvalidInput(format!("invalid journal path: {}", relative.display()))
            })?;
            DocumentKind::Journal(
                JournalDate::from_journal_file_name(file_name)
                    .ok_or_else(|| WriteError::InvalidInput("invalid journal file".to_string()))?,
            )
        } else {
            let page_file = relative.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
                WriteError::InvalidInput(format!("invalid page path: {}", relative.display()))
            })?;
            DocumentKind::Page(
                crate::model::page_key_from_page_file(page_file).ok_or_else(|| {
                    WriteError::InvalidInput(format!("unsupported page file: {}", relative.display()))
                })?,
            )
        };
        let parsed = parse_document(kind, relative, absolute_path.clone(), &content)?;
        let mut replacements = Vec::new();
        for entry in parsed.entries {
            for reference in entry.references {
                if reference.kind == ReferenceKind::Asset {
                    match reference.path.as_deref() {
                        Some(path) if path == old_value => {
                            replacements.push((reference.anchor.span.clone(), new_value.clone()));
                        }
                        Some(path) if path == old_assets_value => {
                            replacements.push((
                                reference.anchor.span.clone(),
                                new_assets_value.clone(),
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }
        if replacements.is_empty() {
            continue;
        }
        file_changes.insert(absolute_path.clone(), apply_replacements(&content, &replacements));
        changed.push(absolute_path);
    }

    let mut originals = BTreeMap::new();
    for (path, new_content) in &file_changes {
        originals.insert(path.clone(), fs::read_to_string(path)?);
        if let Err(error) = atomic_write(path, new_content) {
            for (restore_path, original) in originals {
                let _ = atomic_write(&restore_path, &original);
            }
            return Err(WriteError::Io(error));
        }
    }

    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&old_path, &new_path)?;
    changed.push(old_path);
    changed.push(new_path);
    Ok(changed)
}

fn resolve_anchor_span(
    content: &str,
    anchor: &SourceAnchor,
    expected_snippet: &str,
) -> Result<Span, WriteError> {
    if anchor.span.byte_end <= content.len()
        && &content[anchor.span.byte_start..anchor.span.byte_end] == expected_snippet
    {
        return Ok(anchor.span.clone());
    }

    let matches = find_all(content, expected_snippet);
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    Err(WriteError::StaleAnchor(format!(
        "stale anchor for {}",
        anchor.file_path.display()
    )))
}

fn resolve_span(content: &str, span: &Span, expected: &str) -> Result<Span, WriteError> {
    if span.byte_end <= content.len() && &content[span.byte_start..span.byte_end] == expected {
        return Ok(span.clone());
    }
    let matches = find_all(content, expected);
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    Err(WriteError::StaleAnchor("task marker no longer matches".to_string()))
}

fn find_all(content: &str, needle: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    if needle.is_empty() {
        return spans;
    }
    let mut offset = 0usize;
    while let Some(position) = content[offset..].find(needle) {
        let start = offset + position;
        let end = start + needle.len();
        spans.push(Span {
            byte_start: start,
            byte_end: end,
            line_start: 0,
            line_end: 0,
        });
        offset = end;
    }
    spans
}

fn replace_span(content: &str, span: &Span, replacement: &str) -> String {
    let mut updated = String::new();
    updated.push_str(&content[..span.byte_start]);
    updated.push_str(replacement);
    updated.push_str(&content[span.byte_end..]);
    updated
}

fn serialize_front_matter(front_matter: &FrontMatter, body: &str) -> String {
    if front_matter.values.is_empty() {
        return if body.is_empty() {
            String::new()
        } else {
            format!("{}\n", body.trim_end())
        };
    }
    let mut output = String::new();
    output.push_str("---\n");
    for (key, value) in &front_matter.values {
        match value {
            FrontMatterValue::Scalar(value) => {
                output.push_str(&format!("{key}: {value}\n"));
            }
            FrontMatterValue::List(values) => {
                output.push_str(&format!("{key}:\n"));
                for value in values {
                    output.push_str(&format!("- {value}\n"));
                }
            }
        }
    }
    output.push_str("---\n");
    if !body.trim().is_empty() {
        output.push_str(body.trim_end());
        output.push('\n');
    }
    output
}

fn reference_replacements(
    parsed: &ParsedDocument,
    old_key: &PageKey,
    new_key: &PageKey,
) -> Vec<(Span, String)> {
    let mut replacements = Vec::new();
    for entry in &parsed.entries {
        for reference in &entry.references {
            if reference.page_key.as_ref() != Some(old_key) {
                continue;
            }
            let replacement = match reference.kind {
                ReferenceKind::Tag => format!("#{}", new_key.as_str()),
                ReferenceKind::PageLink => format!("[[{}]]", new_key.as_str()),
                ReferenceKind::Asset => continue,
            };
            replacements.push((reference.anchor.span.clone(), replacement));
        }
    }
    replacements
}

fn apply_replacements(content: &str, replacements: &[(Span, String)]) -> String {
    let mut replacements = replacements.to_vec();
    replacements.sort_by(|left, right| right.0.byte_start.cmp(&left.0.byte_start));
    let mut updated = content.to_string();
    for (span, replacement) in replacements {
        updated.replace_range(span.byte_start..span.byte_end, &replacement);
    }
    updated
}

fn normalize_asset_reference(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = temp_path_for(path);
    fs::write(&temp_path, content)?;

    if path.exists() {
        let backup_path = backup_path_for(path);
        fs::rename(path, &backup_path)?;
        match fs::rename(&temp_path, path) {
            Ok(()) => {
                let _ = fs::remove_file(&backup_path);
                Ok(())
            }
            Err(error) => {
                let _ = fs::rename(&backup_path, path);
                let _ = fs::remove_file(&temp_path);
                Err(error)
            }
        }
    } else {
        fs::rename(temp_path, path)
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_extension(format!("tmp-{stamp}"))
}

fn backup_path_for(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_extension(format!("bak-{stamp}"))
}

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{
    CompatibilityIssue, CompatibilitySeverity, DocumentKind, Entry, EntryKind, FrontMatter,
    FrontMatterValue, InlineReference, JournalDate, PageKey, ParsedDocument, ReferenceKind,
    SourceAnchor, Span, Task, page_key_from_page_file,
};
use crate::workspace::is_journal_file;

pub fn parse_workspace_file(root: &Path, absolute_path: &Path) -> io::Result<ParsedDocument> {
    let content = fs::read_to_string(absolute_path)?;
    let relative_path = absolute_path
        .strip_prefix(root)
        .unwrap_or(absolute_path)
        .to_path_buf();
    let kind = infer_document_kind(&relative_path)?;
    parse_document(kind, relative_path, absolute_path.to_path_buf(), &content)
}

pub fn parse_document(
    kind: DocumentKind,
    relative_path: PathBuf,
    absolute_path: PathBuf,
    content: &str,
) -> io::Result<ParsedDocument> {
    let (front_matter, body, front_matter_line_count) = split_front_matter(content);
    let body_offset = content.len().saturating_sub(body.len());
    let mut entries = Vec::new();
    let mut issues = detect_document_issues(&relative_path, content);
    let line_map = build_line_map(content);
    let body_line_start = front_matter_line_count + 1;
    let mut current_start = None;
    let mut current_end = 0usize;
    let mut current_text = String::new();

    let body_lines: Vec<&str> = body.lines().collect();
    for (index, line) in body_lines.iter().enumerate() {
        let absolute_line = body_line_start + index;
        if line.trim().is_empty() {
            flush_entry(
                &mut entries,
                &mut current_start,
                &mut current_end,
                &mut current_text,
                &absolute_path,
                &relative_path,
                content,
                &line_map,
            );
            continue;
        }

        let trimmed = line.trim_start();
        let entry_is_task = parse_task_marker(trimmed);
        if current_start.is_some() && entry_is_task.is_some() {
            flush_entry(
                &mut entries,
                &mut current_start,
                &mut current_end,
                &mut current_text,
                &absolute_path,
                &relative_path,
                content,
                &line_map,
            );
        }

        let line_start = line_map
            .get(absolute_line.saturating_sub(1))
            .copied()
            .unwrap_or(body_offset);
        let line_end = line_start + line.len();
        if current_start.is_none() {
            current_start = Some(line_start);
            current_end = line_end;
            current_text.push_str(line);
        } else if entry_is_task.is_some() {
            current_start = Some(line_start);
            current_end = line_end;
            current_text.clear();
            current_text.push_str(line);
        } else {
            current_end = line_end;
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }

    flush_entry(
        &mut entries,
        &mut current_start,
        &mut current_end,
        &mut current_text,
        &absolute_path,
        &relative_path,
        content,
        &line_map,
    );

    for entry in &entries {
        for reference in &entry.references {
            if reference.kind == ReferenceKind::PageLink && reference.raw.contains("(((") {
                issues.push(CompatibilityIssue {
                    severity: CompatibilitySeverity::Unsupported,
                    relative_path: relative_path.clone(),
                    construct: "nested block ref".to_string(),
                    message: "block refs are intentionally unsupported".to_string(),
                });
            }
        }
    }

    Ok(ParsedDocument {
        kind,
        relative_path,
        absolute_path,
        front_matter,
        body,
        entries,
        issues,
    })
}

pub fn infer_document_kind(relative_path: &Path) -> io::Result<DocumentKind> {
    if is_journal_file(relative_path) {
        let file_name = relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid journal file"))?;
        let date = JournalDate::from_journal_file_name(file_name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid journal date"))?;
        return Ok(DocumentKind::Journal(date));
    }

    let file_name = relative_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid page file"))?;
    let page_key = page_key_from_page_file(file_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported page filename: {}", relative_path.display()),
        )
    })?;
    Ok(DocumentKind::Page(page_key))
}

fn flush_entry(
    entries: &mut Vec<Entry>,
    current_start: &mut Option<usize>,
    current_end: &mut usize,
    current_text: &mut String,
    absolute_path: &Path,
    relative_path: &Path,
    full_content: &str,
    line_map: &[usize],
) {
    let Some(byte_start) = *current_start else {
        return;
    };
    let byte_end = *current_end;
    let text = current_text.trim_end().to_string();
    if text.is_empty() {
        *current_start = None;
        *current_end = 0;
        current_text.clear();
        return;
    }

    let span = span_from_bytes(full_content, byte_start, byte_end, line_map);
    let anchor = build_anchor(absolute_path, full_content, &span);
    let references = extract_references(relative_path, absolute_path, full_content, &text, byte_start, line_map);
    let task = build_task_if_present(absolute_path, full_content, &text, byte_start, line_map);
    let kind = task
        .as_ref()
        .map(|task| EntryKind::Task {
            checked: task.checked,
        })
        .unwrap_or(EntryKind::Paragraph);

    entries.push(Entry {
        kind,
        text,
        anchor,
        references,
        tasks: task.into_iter().collect(),
    });

    *current_start = None;
    *current_end = 0;
    current_text.clear();
}

fn split_front_matter(content: &str) -> (FrontMatter, String, usize) {
    if !content.starts_with("---\n") {
        return (FrontMatter::default(), content.to_string(), 0);
    }

    let mut lines = content.lines();
    lines.next();
    let mut raw = Vec::new();
    let mut front_matter_line_count = 0usize;
    for line in lines.by_ref() {
        front_matter_line_count += 1;
        if line.trim() == "---" {
            break;
        }
        raw.push(line.to_string());
    }

    let consumed = 2 + front_matter_line_count;
    let body = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n");
    (parse_front_matter_lines(&raw), body, consumed)
}

fn parse_front_matter_lines(lines: &[String]) -> FrontMatter {
    let mut values = BTreeMap::new();
    let mut current_list_key: Option<String> = None;
    let mut current_items = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            current_items.push(item.trim().to_string());
            continue;
        }

        if let Some(key) = current_list_key.take() {
            values.insert(key, FrontMatterValue::List(std::mem::take(&mut current_items)));
        }

        let mut parts = trimmed.splitn(2, ':');
        let key = parts.next().unwrap_or_default().trim();
        let value = parts.next().unwrap_or_default().trim();
        if key.is_empty() {
            continue;
        }
        if value.is_empty() {
            current_list_key = Some(key.to_string());
        } else {
            values.insert(key.to_string(), FrontMatterValue::Scalar(value.to_string()));
        }
    }

    if let Some(key) = current_list_key.take() {
        values.insert(key, FrontMatterValue::List(current_items));
    }

    FrontMatter { values }
}

fn build_line_map(content: &str) -> Vec<usize> {
    let mut lines = vec![0];
    for (index, ch) in content.char_indices() {
        if ch == '\n' {
            lines.push(index + 1);
        }
    }
    lines
}

fn span_from_bytes(content: &str, byte_start: usize, byte_end: usize, line_map: &[usize]) -> Span {
    let mut line_start = 1usize;
    let mut line_end = 1usize;
    for (index, offset) in line_map.iter().enumerate() {
        if *offset <= byte_start {
            line_start = index + 1;
        }
        if *offset <= byte_end {
            line_end = index + 1;
        }
    }
    let _ = content;
    Span {
        byte_start,
        byte_end,
        line_start,
        line_end,
    }
}

fn build_anchor(path: &Path, content: &str, span: &Span) -> SourceAnchor {
    let snippet = content[span.byte_start..span.byte_end].to_string();
    let mut hasher = DefaultHasher::new();
    snippet.hash(&mut hasher);
    SourceAnchor {
        file_path: path.to_path_buf(),
        span: span.clone(),
        snippet,
        snippet_hash: hasher.finish(),
    }
}

fn extract_references(
    relative_path: &Path,
    absolute_path: &Path,
    full_content: &str,
    text: &str,
    entry_byte_start: usize,
    line_map: &[usize],
) -> Vec<InlineReference> {
    let mut references = Vec::new();
    references.extend(extract_page_links(
        absolute_path,
        full_content,
        text,
        entry_byte_start,
        line_map,
    ));
    references.extend(extract_tags(
        absolute_path,
        full_content,
        text,
        entry_byte_start,
        line_map,
    ));
    references.extend(extract_assets(
        absolute_path,
        full_content,
        text,
        entry_byte_start,
        line_map,
    ));
    let _ = relative_path;
    references.sort_by(|left, right| left.anchor.span.byte_start.cmp(&right.anchor.span.byte_start));
    references
}

fn extract_page_links(
    absolute_path: &Path,
    full_content: &str,
    text: &str,
    entry_byte_start: usize,
    line_map: &[usize],
) -> Vec<InlineReference> {
    let mut references = Vec::new();
    let mut offset = 0usize;
    while let Some(start) = text[offset..].find("[[") {
        let absolute_start = offset + start;
        let content_start = absolute_start + 2;
        let Some(end) = text[content_start..].find("]]") else {
            break;
        };
        let absolute_end = content_start + end + 2;
        let raw = &text[content_start..content_start + end];
        let Some(page_key) = PageKey::new(raw) else {
            offset = absolute_end;
            continue;
        };
        let span = span_from_bytes(
            full_content,
            entry_byte_start + absolute_start,
            entry_byte_start + absolute_end,
            line_map,
        );
        references.push(InlineReference {
            kind: ReferenceKind::PageLink,
            raw: raw.to_string(),
            page_key: Some(page_key),
            path: None,
            anchor: build_anchor(absolute_path, full_content, &span),
        });
        offset = absolute_end;
    }
    references
}

fn extract_tags(
    absolute_path: &Path,
    full_content: &str,
    text: &str,
    entry_byte_start: usize,
    line_map: &[usize],
) -> Vec<InlineReference> {
    let mut references = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'#' {
            let prev_is_word = index > 0 && bytes[index - 1].is_ascii_alphanumeric();
            if prev_is_word {
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            while index < bytes.len() {
                let ch = bytes[index] as char;
                if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-') {
                    index += 1;
                } else {
                    break;
                }
            }
            let raw = &text[start + 1..index];
            if let Some(page_key) = PageKey::new(raw) {
                let span = span_from_bytes(
                    full_content,
                    entry_byte_start + start,
                    entry_byte_start + index,
                    line_map,
                );
                references.push(InlineReference {
                    kind: ReferenceKind::Tag,
                    raw: raw.to_string(),
                    page_key: Some(page_key),
                    path: None,
                    anchor: build_anchor(absolute_path, full_content, &span),
                });
            }
            continue;
        }
        index += 1;
    }
    references
}

fn extract_assets(
    absolute_path: &Path,
    full_content: &str,
    text: &str,
    entry_byte_start: usize,
    line_map: &[usize],
) -> Vec<InlineReference> {
    let mut references = Vec::new();
    let patterns = ["![", "["];
    for pattern in patterns {
        let mut offset = 0usize;
        while let Some(start) = text[offset..].find(pattern) {
            let absolute_start = offset + start;
            let Some(open_paren) = text[absolute_start..].find('(') else {
                break;
            };
            let Some(close_paren) = text[absolute_start + open_paren + 1..].find(')') else {
                break;
            };
            let path_start = absolute_start + open_paren + 1;
            let path_end = path_start + close_paren;
            let raw_path = text[path_start..path_end].trim();
            if !raw_path.is_empty() && !raw_path.contains("://") {
                let span = span_from_bytes(
                    full_content,
                    entry_byte_start + path_start,
                    entry_byte_start + path_end,
                    line_map,
                );
                references.push(InlineReference {
                    kind: ReferenceKind::Asset,
                    raw: raw_path.to_string(),
                    page_key: None,
                    path: Some(raw_path.to_string()),
                    anchor: build_anchor(absolute_path, full_content, &span),
                });
            }
            offset = path_end;
        }
    }
    references
}

fn build_task_if_present(
    absolute_path: &Path,
    full_content: &str,
    text: &str,
    entry_byte_start: usize,
    line_map: &[usize],
) -> Option<Task> {
    let trimmed = text.trim_start();
    let (checked, marker_offset) = parse_task_marker(trimmed)?;
    let leading_ws = text.len().saturating_sub(trimmed.len());
    let marker_absolute_start = entry_byte_start + leading_ws + marker_offset;
    let marker_absolute_end = marker_absolute_start + 3;
    let marker_span = span_from_bytes(full_content, marker_absolute_start, marker_absolute_end, line_map);
    let anchor_span = span_from_bytes(
        full_content,
        entry_byte_start,
        entry_byte_start + text.len(),
        line_map,
    );
    let linked_pages = extract_references(
        Path::new(""),
        absolute_path,
        full_content,
        text,
        entry_byte_start,
        line_map,
    )
    .into_iter()
    .filter_map(|reference| reference.page_key)
    .collect();
    Some(Task {
        text: trimmed
            .strip_prefix("- [ ]")
            .or_else(|| trimmed.strip_prefix("- [x]"))
            .or_else(|| trimmed.strip_prefix("- [X]"))
            .unwrap_or(trimmed)
            .trim()
            .to_string(),
        checked,
        linked_pages,
        anchor: build_anchor(absolute_path, full_content, &anchor_span),
        marker_span,
    })
}

fn parse_task_marker(trimmed_line: &str) -> Option<(bool, usize)> {
    if trimmed_line.starts_with("- [ ]") {
        Some((false, 2))
    } else if trimmed_line.starts_with("- [x]") || trimmed_line.starts_with("- [X]") {
        Some((true, 2))
    } else {
        None
    }
}

fn detect_document_issues(relative_path: &Path, content: &str) -> Vec<CompatibilityIssue> {
    let mut issues = Vec::new();
    let patterns = [
        (
            "manual block ref",
            CompatibilitySeverity::Unsupported,
            "((",
            "manual block refs are intentionally unsupported",
        ),
        (
            "manual block embed",
            CompatibilitySeverity::Unsupported,
            "{{embed",
            "block embeds are intentionally unsupported",
        ),
        (
            "durable block id",
            CompatibilitySeverity::Degraded,
            "id::",
            "durable block IDs degrade to file-and-span anchors",
        ),
    ];
    for (construct, severity, pattern, message) in patterns {
        if content.contains(pattern) {
            issues.push(CompatibilityIssue {
                severity: severity.clone(),
                relative_path: relative_path.to_path_buf(),
                construct: construct.to_string(),
                message: message.to_string(),
            });
        }
    }
    issues
}

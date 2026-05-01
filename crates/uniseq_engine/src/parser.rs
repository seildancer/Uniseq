use crate::model::*;
use crate::page_identity::{filename_to_page_path, normalize_page_path};
use chrono::NaiveDate;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported document path: {0}")]
    UnsupportedPath(PathBuf),
}

pub fn parse_markdown_file(
    path: impl AsRef<Path>,
    root: impl AsRef<Path>,
) -> Result<ParsedDocument, ParseError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let kind = document_kind(path, root.as_ref())?;
    Ok(parse_markdown_text(path.to_path_buf(), kind, &text))
}

pub fn parse_markdown_text(path: PathBuf, kind: DocumentKind, text: &str) -> ParsedDocument {
    let (front_matter, body_start) = extract_front_matter(text);
    let aliases = front_matter
        .as_deref()
        .map(extract_aliases)
        .unwrap_or_default();
    let body = &text[body_start..];
    let link_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    let tag_re = Regex::new(r"(^|\s)#([A-Za-z0-9][A-Za-z0-9_/-]*)").unwrap();
    let mut entries = Vec::new();
    let mut line_start = body_start;

    for line in body.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches(['\r', '\n']);
        if !line_without_newline.trim().is_empty()
            && !line_without_newline.trim_start().starts_with("```")
        {
            let entry_start = line_start;
            let entry_end = line_start + line_without_newline.len();
            let raw = line_without_newline.to_string();
            let (level, task, display) = classify_line(line_without_newline);
            let links = link_re
                .captures_iter(line_without_newline)
                .filter_map(|cap| {
                    let m = cap.get(0)?;
                    let raw_target = cap.get(1)?.as_str().trim().to_string();
                    Some(PageRef {
                        raw: raw_target.clone(),
                        page_path: normalize_page_path(&raw_target),
                        span: Span {
                            start: entry_start + m.start(),
                            end: entry_start + m.end(),
                        },
                    })
                })
                .collect::<Vec<_>>();
            let tags = tag_re
                .captures_iter(line_without_newline)
                .filter_map(|cap| {
                    let m = cap.get(2)?;
                    let raw_target = m.as_str().to_string();
                    Some(PageRef {
                        raw: raw_target.clone(),
                        page_path: normalize_page_path(&raw_target),
                        span: Span {
                            start: entry_start + m.start(),
                            end: entry_start + m.end(),
                        },
                    })
                })
                .collect::<Vec<_>>();
            let anchor = SourceAnchor {
                file_path: path.clone(),
                span: Span {
                    start: entry_start,
                    end: entry_end,
                },
                snippet: raw.clone(),
            };
            entries.push(Entry {
                runtime_id: format!("{}:{}-{}", path.display(), entry_start, entry_end),
                text: display,
                level,
                task,
                links,
                tags,
                anchor,
            });
        }
        line_start += line.len();
    }

    ParsedDocument {
        kind,
        path,
        front_matter,
        aliases,
        entries,
    }
}

fn document_kind(path: &Path, root: &Path) -> Result<DocumentKind, ParseError> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts = rel.iter().filter_map(|p| p.to_str()).collect::<Vec<_>>();
    if parts.len() == 2 && parts[0] == "journals" {
        let stem = Path::new(parts[1])
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ParseError::UnsupportedPath(path.to_path_buf()))?;
        let date = NaiveDate::parse_from_str(stem, "%Y-%m-%d")
            .map_err(|_| ParseError::UnsupportedPath(path.to_path_buf()))?;
        return Ok(DocumentKind::Journal { date });
    }
    if parts.len() == 2 && parts[0] == "pages" {
        if let Some(page_path) = filename_to_page_path(parts[1]) {
            return Ok(DocumentKind::Page { page_path });
        }
    }
    Err(ParseError::UnsupportedPath(path.to_path_buf()))
}

fn extract_front_matter(text: &str) -> (Option<String>, usize) {
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") {
        return (None, 0);
    }
    let mut offset = if text.starts_with("---\r\n") { 5 } else { 4 };
    for line in text[offset..].split_inclusive('\n') {
        if line.trim() == "---" {
            let end = offset + line.len();
            return (Some(text[..end].to_string()), end);
        }
        offset += line.len();
    }
    (None, 0)
}

fn extract_aliases(front_matter: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    let mut in_aliases = false;
    for line in front_matter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("aliases:") {
            in_aliases = true;
            let inline = trimmed.trim_start_matches("aliases:").trim();
            if !inline.is_empty() {
                aliases.push(inline.trim_matches(['[', ']', '"', '\'']).to_string());
            }
            continue;
        }
        if in_aliases && trimmed.starts_with('-') {
            aliases.push(
                trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches(['"', '\''])
                    .to_string(),
            );
        } else if in_aliases && !trimmed.is_empty() && !line.starts_with(' ') {
            in_aliases = false;
        }
    }
    aliases.into_iter().filter(|a| !a.is_empty()).collect()
}

fn classify_line(line: &str) -> (EntryLevel, Option<TaskState>, String) {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("#") {
        let count = trimmed.chars().take_while(|c| *c == '#').count().min(6) as u8;
        let text = rest.trim_start_matches('#').trim().to_string();
        return (EntryLevel::Heading(count), None, text);
    }
    let ordered_re = Regex::new(r"^\d+\. ").unwrap();
    let list_prefix =
        trimmed.starts_with("- ") || trimmed.starts_with("* ") || ordered_re.is_match(trimmed);
    let task = if trimmed.starts_with("- [ ] ") {
        Some(TaskState::Todo)
    } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
        Some(TaskState::Done)
    } else {
        None
    };
    let display = if let Some(m) = ordered_re.find(trimmed) {
        trimmed[m.end()..].to_string()
    } else {
        trimmed
            .trim_start_matches("- [ ] ")
            .trim_start_matches("- [x] ")
            .trim_start_matches("- [X] ")
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .to_string()
    };
    (
        if list_prefix {
            EntryLevel::ListItem
        } else {
            EntryLevel::Paragraph
        },
        task,
        display,
    )
}

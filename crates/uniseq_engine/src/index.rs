use crate::model::*;
use crate::page_identity::normalize_page_path;
use crate::parser::{parse_markdown_file, ParseError};
use crate::workspace::{open_workspace, WorkspaceError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub root: PathBuf,
    pub documents: Vec<ParsedDocument>,
    pub pages: BTreeMap<String, IndexedPage>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexedPage {
    pub page_path: String,
    pub has_file: bool,
    pub aliases: Vec<String>,
    pub own_entry_ids: Vec<String>,
    pub incoming_entry_ids: Vec<String>,
    pub outbound_pages: BTreeSet<String>,
}

pub fn build_index(root: impl AsRef<Path>) -> Result<WorkspaceIndex, IndexError> {
    let summary = open_workspace(&root)?;
    let mut documents = Vec::new();
    for journal in &summary.journals {
        documents.push(parse_markdown_file(&journal.path, &summary.root)?);
    }
    for page in &summary.pages {
        documents.push(parse_markdown_file(&page.path, &summary.root)?);
    }

    let mut pages = BTreeMap::<String, IndexedPage>::new();
    let mut entries = Vec::new();
    for doc in &documents {
        if let DocumentKind::Page { page_path } = &doc.kind {
            let page = pages
                .entry(page_path.clone())
                .or_insert_with(|| IndexedPage {
                    page_path: page_path.clone(),
                    ..Default::default()
                });
            page.has_file = true;
            page.aliases = doc.aliases.clone();
            for alias in &doc.aliases {
                let alias_path = normalize_page_path(alias);
                pages
                    .entry(alias_path.clone())
                    .or_insert_with(|| IndexedPage {
                        page_path: alias_path,
                        ..Default::default()
                    });
            }
        }
        for entry in &doc.entries {
            let current_page = match &doc.kind {
                DocumentKind::Page { page_path } => Some(page_path.clone()),
                DocumentKind::Journal { .. } => None,
            };
            if let Some(page_path) = current_page {
                pages
                    .entry(page_path.clone())
                    .or_insert_with(|| IndexedPage {
                        page_path: page_path.clone(),
                        ..Default::default()
                    })
                    .own_entry_ids
                    .push(entry.runtime_id.clone());
            }
            for target in entry.links.iter().chain(entry.tags.iter()) {
                let page = pages
                    .entry(target.page_path.clone())
                    .or_insert_with(|| IndexedPage {
                        page_path: target.page_path.clone(),
                        ..Default::default()
                    });
                page.incoming_entry_ids.push(entry.runtime_id.clone());
                if let Some(source_page) = match &doc.kind {
                    DocumentKind::Page { page_path } => Some(page_path),
                    _ => None,
                } {
                    pages
                        .entry(source_page.clone())
                        .or_insert_with(|| IndexedPage {
                            page_path: source_page.clone(),
                            ..Default::default()
                        })
                        .outbound_pages
                        .insert(target.page_path.clone());
                }
            }
            entries.push(entry.clone());
        }
    }

    Ok(WorkspaceIndex {
        root: summary.root,
        documents,
        pages,
        entries,
    })
}

pub fn query_journal(index: &WorkspaceIndex, date: chrono::NaiveDate) -> Vec<Entry> {
    index
        .documents
        .iter()
        .find_map(|doc| match &doc.kind {
            DocumentKind::Journal { date: d } if *d == date => Some(doc.entries.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn query_page(index: &WorkspaceIndex, page_path: &str) -> PageProjection {
    let page_path = normalize_page_path(page_path);
    let by_id = index
        .entries
        .iter()
        .map(|e| (e.runtime_id.as_str(), e))
        .collect::<BTreeMap<_, _>>();
    let indexed = index
        .pages
        .get(&page_path)
        .cloned()
        .unwrap_or_else(|| IndexedPage {
            page_path: page_path.clone(),
            ..Default::default()
        });
    PageProjection {
        page_path,
        own_entries: indexed
            .own_entry_ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).map(|e| (*e).clone()))
            .collect(),
        incoming_entries: indexed
            .incoming_entry_ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).map(|e| (*e).clone()))
            .collect(),
        aliases: indexed.aliases,
    }
}

pub fn task_rollup(index: &WorkspaceIndex) -> Vec<Entry> {
    index
        .entries
        .iter()
        .filter(|e| e.task.is_some())
        .cloned()
        .collect()
}

pub fn search(index: &WorkspaceIndex, query: &str) -> Vec<SearchHit> {
    let needles = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if needles.is_empty() {
        return Vec::new();
    }
    let mut hits = index
        .entries
        .iter()
        .filter_map(|entry| {
            let text = entry.text.to_lowercase();
            let score = needles.iter().filter(|n| text.contains(n.as_str())).count();
            (score > 0).then(|| SearchHit {
                entry: entry.clone(),
                score,
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.entry.anchor.file_path.cmp(&b.entry.anchor.file_path))
    });
    hits
}

/// Timeline entry with its journal date for grouping/sorting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub date: chrono::NaiveDate,
    pub entry: Entry,
}

/// Returns all entries that belong to journals, grouped by date and sorted newest-first.
/// Each journal's entries appear in document order.
pub fn query_timeline(index: &WorkspaceIndex) -> Vec<TimelineEntry> {
    let mut result = Vec::new();
    for doc in &index.documents {
        if let DocumentKind::Journal { date } = &doc.kind {
            for entry in &doc.entries {
                result.push(TimelineEntry {
                    date: *date,
                    entry: entry.clone(),
                });
            }
        }
    }
    // Sort newest journal first, then by entry order within each journal (stable)
    result.sort_by(|a, b| b.date.cmp(&a.date));
    result
}

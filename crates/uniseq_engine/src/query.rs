//! Structured query API with filters and options.
//!
//! Complements the existing `search` and `task_rollup` with typed filter options.

use crate::index::WorkspaceIndex;
use crate::model::{Entry, TaskState};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Options for structured search.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchOptions {
    /// Free-text query (empty = match all).
    pub text: Option<String>,
    /// Limit to entries in these page paths.
    pub page_filters: Vec<String>,
    /// Limit to entries linked to these tags.
    pub tag_filters: Vec<String>,
    /// Filter by task state.
    pub task_state: Option<TaskStateFilter>,
    /// Limit to entries on or after this date (journal date).
    pub date_from: Option<NaiveDate>,
    /// Limit to entries on or before this date.
    pub date_to: Option<NaiveDate>,
    /// Include assets in results (reserved for future asset entries).
    pub include_assets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStateFilter {
    Todo,
    Done,
    Any,
}

/// Extended search result with filter context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entries: Vec<Entry>,
    pub total: usize,
    pub matched_on: Vec<String>,
}

fn journal_date_of(entry: &Entry, index: &WorkspaceIndex) -> Option<NaiveDate> {
    index.documents.iter()
        .find(|d| d.path == entry.anchor.file_path)
        .and_then(|d| match &d.kind { crate::model::DocumentKind::Journal { date } => Some(*date), _ => None })
}

/// Search with structured options on top of a WorkspaceIndex.
pub fn search_with_options(index: &WorkspaceIndex, opts: &SearchOptions) -> SearchResult {
    let text_needles: Vec<String> = opts.text.as_ref()
        .map(|t| t.to_lowercase().split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();

    let filtered: Vec<_> = index.entries.iter().filter(|entry| {
        if !text_needles.is_empty() {
            let text_lower = entry.text.to_lowercase();
            if !text_needles.iter().all(|n| text_lower.contains(n)) {
                return false;
            }
        }
        if let Some(ref f) = opts.task_state {
            match f {
                TaskStateFilter::Todo => {
                    if entry.task != Some(TaskState::Todo) { return false; }
                }
                TaskStateFilter::Done => {
                    if entry.task != Some(TaskState::Done) { return false; }
                }
                TaskStateFilter::Any => {}
            }
        }
        if !opts.page_filters.is_empty() {
            let entry_pages: BTreeSet<_> = entry.links.iter().map(|l| l.page_path.clone()).collect();
            if !opts.page_filters.iter().any(|pf| entry_pages.contains(pf)) {
                return false;
            }
        }
        if !opts.tag_filters.is_empty() {
            let entry_tags: BTreeSet<_> = entry.tags.iter().map(|t| t.page_path.clone()).collect();
            if !opts.tag_filters.iter().any(|tf| entry_tags.contains(tf)) {
                return false;
            }
        }
        if opts.date_from.is_some() || opts.date_to.is_some() {
            if let Some(d) = journal_date_of(entry, index) {
                if let Some(from) = opts.date_from {
                    if d < from { return false; }
                }
                if let Some(to) = opts.date_to {
                    if d > to { return false; }
                }
            } else {
                return false;
            }
        }
        true
    }).cloned().collect();

    let total = filtered.len();
    SearchResult { entries: filtered, total, matched_on: text_needles }
}

/// Task query with typed filters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskQuery {
    pub state: Option<TaskStateFilter>,
    pub page: Option<String>,
    pub tag: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

pub fn task_query(index: &WorkspaceIndex, q: TaskQuery) -> Vec<Entry> {
    index.entries.iter().filter(|entry| {
        if entry.task.is_none() { return false; }
        match (&q.state, &entry.task) {
            (Some(TaskStateFilter::Todo), Some(TaskState::Todo)) => {}
            (Some(TaskStateFilter::Done), Some(TaskState::Done)) => {}
            (Some(TaskStateFilter::Any), _) => {}
            (None, _) => {}
            _ => return false,
        }
        if let Some(ref page) = q.page {
            let linked_pages: BTreeSet<_> = entry.links.iter().map(|l| &l.page_path).collect();
            if !linked_pages.contains(page) { return false; }
        }
        if let Some(ref tag) = q.tag {
            let entry_tags: BTreeSet<_> = entry.tags.iter().map(|t| &t.page_path).collect();
            if !entry_tags.contains(tag) { return false; }
        }
        if let Some(from) = q.date_from {
            if let Some(d) = journal_date_of(entry, index) {
                if d < from { return false; }
            }
        }
        if let Some(to) = q.date_to {
            if let Some(d) = journal_date_of(entry, index) {
                if d > to { return false; }
            }
        }
        true
    }).cloned().collect()
}
use crate::index::WorkspaceIndex;
use crate::model::{IncomingReference, JournalDate, PageKey, PageView, SearchHit, Task, TimelineEntry};

pub fn journal_dates(index: &WorkspaceIndex) -> Vec<JournalDate> {
    index.journal_entries.keys().cloned().collect()
}

pub fn page_view(index: &WorkspaceIndex, page_key: &PageKey) -> Option<PageView> {
    let page = index.pages.get(page_key)?.clone();
    let page_body = page
        .page_file
        .as_ref()
        .and_then(|path| index.documents.get(path))
        .map(|document| document.body.clone())
        .filter(|body| !body.trim().is_empty());
    let incoming = index
        .incoming
        .get(page_key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|edge| IncomingReference { edge })
        .collect();
    let open_tasks = index
        .tasks
        .iter()
        .filter(|task| !task.checked && task.linked_pages.iter().any(|linked| linked == page_key))
        .cloned()
        .collect();
    let related_pages = index.pages_related_to(page_key);

    Some(PageView {
        page,
        page_body,
        incoming,
        open_tasks,
        related_pages,
    })
}

pub fn open_tasks(index: &WorkspaceIndex, page_key: Option<&PageKey>) -> Vec<Task> {
    index
        .tasks
        .iter()
        .filter(|task| !task.checked)
        .filter(|task| {
            page_key.map_or(true, |page_key| task.linked_pages.iter().any(|linked| linked == page_key))
        })
        .cloned()
        .collect()
}

pub fn timeline(index: &WorkspaceIndex, page_key: Option<&PageKey>) -> Vec<TimelineEntry> {
    let mut entries = Vec::new();
    for document in index.documents.values() {
        for entry in &document.entries {
            let matches_page = page_key.map_or(true, |page_key| {
                entry.references
                    .iter()
                    .filter_map(|reference| reference.page_key.as_ref())
                    .any(|linked| linked == page_key)
            });
            if !matches_page {
                continue;
            }
            let (source_date, source_page) = match &document.kind {
                crate::model::DocumentKind::Journal(date) => (Some(date.clone()), None),
                crate::model::DocumentKind::Page(page_key) => (None, Some(page_key.clone())),
            };
            entries.push(TimelineEntry {
                anchor: entry.anchor.clone(),
                source_date,
                source_page,
                text: entry.text.clone(),
            });
        }
    }
    entries.sort_by(|left, right| {
        right
            .source_date
            .cmp(&left.source_date)
            .then_with(|| left.anchor.span.byte_start.cmp(&right.anchor.span.byte_start))
    });
    entries
}

pub fn search(index: &WorkspaceIndex, query: &str) -> Vec<SearchHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut hits = Vec::new();
    for document in index.documents.values() {
        for entry in &document.entries {
            if !entry.text.to_lowercase().contains(&needle) {
                continue;
            }
            let linked_pages = entry
                .references
                .iter()
                .filter_map(|reference| reference.page_key.clone())
                .collect();
            let source_date = match &document.kind {
                crate::model::DocumentKind::Journal(date) => Some(date.clone()),
                crate::model::DocumentKind::Page(_) => None,
            };
            hits.push(SearchHit {
                path: document.relative_path.clone(),
                anchor: entry.anchor.clone(),
                excerpt: excerpt(&entry.text, &needle),
                source_date,
                linked_pages,
            });
        }
    }
    hits
}

fn excerpt(text: &str, needle: &str) -> String {
    let lower = text.to_lowercase();
    if let Some(position) = lower.find(needle) {
        let start = position.saturating_sub(20);
        let end = usize::min(position + needle.len() + 40, text.len());
        text[start..end].to_string()
    } else {
        text.chars().take(60).collect()
    }
}

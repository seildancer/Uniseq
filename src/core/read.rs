use std::cmp::Reverse;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    Block, BlockKind, CoreError, FileFingerprint, Page, PageId, PageLocation, SourceSpan,
    WorkspaceCache,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatBlockSnapshot {
    pub kind: BlockKind,
    pub depth: u32,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageContentSnapshot {
    pub revision: FileFingerprint,
    pub blocks: Vec<FlatBlockSnapshot>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummary {
    pub page_id: PageId,
    pub location: PageLocation,
    pub workspace_path: PathBuf,
    pub title: String,
    pub revision: FileFingerprint,
    pub modified_at: Option<u64>,
    pub parent_page_id: Option<PageId>,
    pub child_page_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDetail {
    pub summary: PageSummary,
    pub incoming_refs: Vec<IncomingPageRefSnapshot>,
    pub outgoing_refs: Vec<OutgoingPageRefSnapshot>,
    pub outgoing_ref_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub page_id: PageId,
    pub title: String,
    pub location: PageLocation,
    pub matched_field: SearchMatchField,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SearchMatchField {
    Title,
    PageId,
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingPageRefSnapshot {
    pub target_page_id: PageId,
    pub source_page_id: PageId,
    pub source_page_fingerprint: FileFingerprint,
    pub source_block_span: SourceSpan,
    pub ref_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingPageRefSnapshot {
    pub target_page_id: PageId,
    pub ref_span: SourceSpan,
    pub target_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHandle {
    pub source_page_id: PageId,
    pub source_page_revision: FileFingerprint,
    pub block_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSnapshot {
    pub handle: BlockHandle,
    pub kind: BlockKind,
    pub block_span: SourceSpan,
    pub content_span: SourceSpan,
    pub content: String,
    pub markdown: String,
    pub outgoing_refs: Vec<OutgoingPageRefSnapshot>,
    pub children: Vec<BlockSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefHighlightSnapshot {
    pub prefix: String,
    pub highlight: String,
    pub suffix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRefEntry {
    pub target_page_id: PageId,
    pub source_page_id: PageId,
    pub ref_span: SourceSpan,
    pub block: BlockSnapshot,
    pub block_content_highlight: Option<RefHighlightSnapshot>,
}

#[derive(Clone, Copy)]
pub struct WorkspaceReadApi<'a> {
    cache: &'a WorkspaceCache,
    page_modified_at: &'a dyn Fn(&Page) -> Option<SystemTime>,
}

impl<'a> WorkspaceReadApi<'a> {
    pub fn new(
        cache: &'a WorkspaceCache,
        page_modified_at: &'a dyn Fn(&Page) -> Option<SystemTime>,
    ) -> Self {
        Self {
            cache,
            page_modified_at,
        }
    }

    pub fn all_pages(&self) -> Vec<PageSummary> {
        self.cache
            .pages()
            .values()
            .map(|page| page_summary(page, self.page_modified_at))
            .collect()
    }

    pub fn page_summary(&self, page_id: &PageId) -> Result<PageSummary, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(page_summary(page, self.page_modified_at))
    }

    pub fn page_detail(&self, page_id: &PageId) -> Result<PageDetail, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(PageDetail {
            summary: page_summary(page, self.page_modified_at),
            incoming_refs: incoming_ref_snapshots(self.cache, page_id)?,
            outgoing_refs: outgoing_ref_snapshots(self.cache, page),
            outgoing_ref_count: page.outgoing_refs().count(),
        })
    }

    pub fn search_pages(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut matches = self
            .cache
            .pages()
            .values()
            .filter_map(|page| search_result_for_page(page, &normalized_query))
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then_with(|| left.match_position.cmp(&right.match_position))
                .then_with(|| left.title.as_str().cmp(right.title.as_str()))
                .then_with(|| left.page_id.cmp(&right.page_id))
        });

        matches
            .into_iter()
            .take(limit)
            .map(|entry| SearchResult {
                page_id: entry.page_id,
                title: entry.title,
                location: entry.location,
                matched_field: entry.matched_field,
                snippet: entry.snippet,
            })
            .collect()
    }

    pub fn child_pages(&self, page_id: &PageId) -> Result<Vec<PageSummary>, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        page.child_page_ids
            .iter()
            .map(|child_page_id| {
                self.cache
                    .page(child_page_id)
                    .map(|page| page_summary(page, self.page_modified_at))
                    .ok_or(CoreError::MissingPage)
            })
            .collect()
    }

    pub fn page_content(&self, page_id: &PageId) -> Result<PageContentSnapshot, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(PageContentSnapshot {
            revision: page.fingerprint,
            blocks: flat_blocks(page)?,
            text: page.text.clone(),
        })
    }

    pub fn page_incoming_refs(
        &self,
        target_page_id: &PageId,
    ) -> Result<Vec<IncomingPageRefSnapshot>, CoreError> {
        incoming_ref_snapshots(self.cache, target_page_id)
    }

    pub fn page_outgoing_refs(
        &self,
        source_page_id: &PageId,
    ) -> Result<Vec<OutgoingPageRefSnapshot>, CoreError> {
        let source_page = self
            .cache
            .page(source_page_id)
            .ok_or(CoreError::MissingPage)?;
        Ok(outgoing_ref_snapshots(self.cache, source_page))
    }

    pub fn block_snapshot(&self, handle: &BlockHandle) -> Result<BlockSnapshot, CoreError> {
        let source_page = self
            .cache
            .page(&handle.source_page_id)
            .ok_or(CoreError::MissingPage)?;
        let block = source_page.find_block_by_span(handle.block_span).ok_or(
            CoreError::StructuralConflict {
                path: source_page.workspace_path.clone(),
            },
        )?;
        block_snapshot(self.cache, source_page, block)
    }

    pub fn page_linked_refs(
        &self,
        target_page_id: &PageId,
    ) -> Result<Vec<LinkedRefEntry>, CoreError> {
        self.cache
            .page(target_page_id)
            .ok_or(CoreError::MissingPage)?;
        let incoming_refs = self.cache.incoming_refs(target_page_id);
        incoming_refs
            .iter()
            .map(|incoming_ref| {
                let source_page = self
                    .cache
                    .page(&incoming_ref.source_page_id)
                    .ok_or(CoreError::MissingPage)?;
                let block = source_page
                    .find_block_by_span(incoming_ref.source_block_span)
                    .ok_or(CoreError::StructuralConflict {
                        path: source_page.workspace_path.clone(),
                    })?;
                Ok(LinkedRefEntry {
                    target_page_id: target_page_id.clone(),
                    source_page_id: incoming_ref.source_page_id.clone(),
                    ref_span: incoming_ref.ref_span,
                    block: block_snapshot(self.cache, source_page, block)?,
                    block_content_highlight: linked_ref_highlight(
                        source_page,
                        block.content_span,
                        incoming_ref.ref_span,
                    )?,
                })
            })
            .collect()
    }

    pub fn pages_with_missing_targets(&self) -> Vec<PageSummary> {
        self.cache
            .pages()
            .values()
            .filter(|page| {
                page.outgoing_refs()
                    .any(|outgoing_ref| self.cache.page(&outgoing_ref.target_page_id).is_none())
            })
            .map(|page| page_summary(page, self.page_modified_at))
            .collect()
    }

    pub fn all_pages_paginated(&self, offset: usize, limit: usize) -> Vec<PageSummary> {
        self.cache
            .pages()
            .values()
            .skip(offset)
            .take(limit)
            .map(|page| page_summary(page, self.page_modified_at))
            .collect()
    }
}

#[derive(Debug, Clone)]
struct RankedSearchResult {
    page_id: PageId,
    title: String,
    location: PageLocation,
    matched_field: SearchMatchField,
    snippet: Option<String>,
    score: Reverse<u8>,
    match_position: usize,
}

fn flat_blocks(page: &Page) -> Result<Vec<FlatBlockSnapshot>, CoreError> {
    let mut result = Vec::new();
    for block in &page.blocks {
        collect_flat(page, block, 0, &mut result)?;
    }
    Ok(result)
}

fn collect_flat(
    page: &Page,
    block: &Block,
    depth: u32,
    result: &mut Vec<FlatBlockSnapshot>,
) -> Result<(), CoreError> {
    result.push(FlatBlockSnapshot {
        kind: block.kind,
        depth,
        content: resolved_block_content(page, block)?,
    });
    for child in &block.children {
        collect_flat(page, child, depth + 1, result)?;
    }
    Ok(())
}

fn search_result_for_page(page: &Page, normalized_query: &str) -> Option<RankedSearchResult> {
    let title_lower = page.title.to_lowercase();
    if title_lower == normalized_query {
        return Some(RankedSearchResult {
            page_id: page.page_id.clone(),
            title: page.title.clone(),
            location: page.location.clone(),
            matched_field: SearchMatchField::Title,
            snippet: None,
            score: Reverse(6),
            match_position: 0,
        });
    }

    if let Some(position) = title_lower.find(normalized_query) {
        return Some(RankedSearchResult {
            page_id: page.page_id.clone(),
            title: page.title.clone(),
            location: page.location.clone(),
            matched_field: SearchMatchField::Title,
            snippet: None,
            score: Reverse(if position == 0 { 5 } else { 4 }),
            match_position: position,
        });
    }

    let page_id_string = page.page_id.to_string();
    let page_id_lower = page_id_string.to_lowercase();
    if let Some(position) = page_id_lower.find(normalized_query) {
        return Some(RankedSearchResult {
            page_id: page.page_id.clone(),
            title: page.title.clone(),
            location: page.location.clone(),
            matched_field: SearchMatchField::PageId,
            snippet: None,
            score: Reverse(3),
            match_position: position,
        });
    }

    let text_lower = page.text.to_lowercase();
    text_lower
        .find(normalized_query)
        .map(|position| RankedSearchResult {
            page_id: page.page_id.clone(),
            title: page.title.clone(),
            location: page.location.clone(),
            matched_field: SearchMatchField::Content,
            snippet: Some(build_search_snippet(
                &page.text,
                position,
                normalized_query.len(),
            )),
            score: Reverse(2),
            match_position: position,
        })
}

fn build_search_snippet(text: &str, match_start: usize, match_len: usize) -> String {
    let chars = text.char_indices().collect::<Vec<_>>();
    let char_count = text.chars().count();
    let start_char = text[..match_start].chars().count().saturating_sub(30);
    let match_end_char = text[..match_start + match_len].chars().count();
    let end_char = (match_end_char + 30).min(char_count);

    let start_byte = if start_char == 0 {
        0
    } else {
        chars[start_char].0
    };
    let end_byte = if end_char >= char_count {
        text.len()
    } else {
        chars[end_char].0
    };

    let prefix = if start_byte > 0 { "..." } else { "" };
    let suffix = if end_byte < text.len() { "..." } else { "" };
    format!(
        "{prefix}{}{suffix}",
        text[start_byte..end_byte].replace(['\r', '\n'], " ")
    )
}

fn incoming_ref_snapshots(
    cache: &WorkspaceCache,
    target_page_id: &PageId,
) -> Result<Vec<IncomingPageRefSnapshot>, CoreError> {
    cache.page(target_page_id).ok_or(CoreError::MissingPage)?;
    let incoming_refs = cache.incoming_refs(target_page_id);
    Ok(incoming_refs
        .iter()
        .map(|incoming_ref| IncomingPageRefSnapshot {
            target_page_id: target_page_id.clone(),
            source_page_id: incoming_ref.source_page_id.clone(),
            source_page_fingerprint: incoming_ref.source_page_fingerprint,
            source_block_span: incoming_ref.source_block_span,
            ref_span: incoming_ref.ref_span,
        })
        .collect())
}

fn page_summary(
    page: &Page,
    page_modified_at: &dyn Fn(&Page) -> Option<SystemTime>,
) -> PageSummary {
    PageSummary {
        page_id: page.page_id.clone(),
        location: page.location.clone(),
        workspace_path: page.workspace_path.clone(),
        title: page.title.clone(),
        revision: page.fingerprint,
        modified_at: page_modified_at(page).and_then(system_time_to_unix_ms),
        parent_page_id: page.parent_page_id(),
        child_page_count: page.child_page_ids.len(),
    }
}

fn system_time_to_unix_ms(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn outgoing_ref_snapshots(
    cache: &WorkspaceCache,
    source_page: &Page,
) -> Vec<OutgoingPageRefSnapshot> {
    source_page
        .outgoing_refs()
        .map(|outgoing_ref| OutgoingPageRefSnapshot {
            target_page_id: outgoing_ref.target_page_id.clone(),
            ref_span: outgoing_ref.ref_span,
            target_exists: cache.page(&outgoing_ref.target_page_id).is_some(),
        })
        .collect()
}

fn resolved_block_content(source_page: &Page, block: &Block) -> Result<String, CoreError> {
    let content = block.content_span.slice(&source_page.text)?;
    Ok(content
        .strip_suffix("\r\n")
        .or_else(|| content.strip_suffix('\n'))
        .unwrap_or(content)
        .to_owned())
}

fn resolved_block_markdown(source_page: &Page, block: &Block) -> Result<String, CoreError> {
    Ok(block.block_span.slice(&source_page.text)?.to_owned())
}

fn block_snapshot(
    cache: &WorkspaceCache,
    source_page: &Page,
    block: &Block,
) -> Result<BlockSnapshot, CoreError> {
    Ok(BlockSnapshot {
        handle: BlockHandle {
            source_page_id: source_page.page_id.clone(),
            source_page_revision: source_page.fingerprint,
            block_span: block.block_span,
        },
        kind: block.kind,
        block_span: block.block_span,
        content_span: block.content_span,
        content: resolved_block_content(source_page, block)?,
        markdown: resolved_block_markdown(source_page, block)?,
        outgoing_refs: block
            .outgoing_refs
            .iter()
            .map(|outgoing_ref| OutgoingPageRefSnapshot {
                target_page_id: outgoing_ref.target_page_id.clone(),
                ref_span: outgoing_ref.ref_span,
                target_exists: cache.page(&outgoing_ref.target_page_id).is_some(),
            })
            .collect(),
        children: block
            .children
            .iter()
            .map(|child| block_snapshot(cache, source_page, child))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn linked_ref_highlight(
    source_page: &Page,
    content_span: SourceSpan,
    ref_span: SourceSpan,
) -> Result<Option<RefHighlightSnapshot>, CoreError> {
    if !content_span.contains_span(ref_span) {
        return Ok(None);
    }

    let prefix_span = SourceSpan::new(content_span.start(), ref_span.start())?;
    let suffix_span = SourceSpan::new(ref_span.end(), content_span.end())?;
    let suffix = suffix_span.slice(&source_page.text)?;
    Ok(Some(RefHighlightSnapshot {
        prefix: prefix_span.slice(&source_page.text)?.to_owned(),
        highlight: ref_span.slice(&source_page.text)?.to_owned(),
        suffix: suffix
            .strip_suffix("\r\n")
            .or_else(|| suffix.strip_suffix('\n'))
            .unwrap_or(suffix)
            .to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Page, parse_blocks};

    fn parsed_page(id: &[&str], text: &str) -> Page {
        Page::new(PageId::new(id.iter().copied()).unwrap(), text)
            .with_blocks(parse_blocks(text).unwrap())
    }

    fn stream_page(stream_name: &str, date_name: &str, text: &str) -> Page {
        let stream_name = crate::PageName::new(stream_name).unwrap();
        let date_name = crate::PageName::new(date_name).unwrap();
        Page::new_in_location(
            PageId::stream(stream_name.clone(), date_name.clone()).unwrap(),
            crate::PageLocation::Stream { stream_name },
            text,
        )
        .unwrap()
        .with_blocks(parse_blocks(text).unwrap())
    }

    #[test]
    fn lists_pages_as_stable_summaries() {
        let cache = WorkspaceCache::from_pages([
            parsed_page(&["A"], ""),
            parsed_page(&["A", "B"], ""),
            parsed_page(&["C"], ""),
        ]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        assert_eq!(
            read_api.all_pages(),
            vec![
                PageSummary {
                    page_id: PageId::new(["A"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("A.md"),
                    title: "A".to_owned(),
                    revision: FileFingerprint::from_text(""),
                    modified_at: None,
                    parent_page_id: None,
                    child_page_count: 1,
                },
                PageSummary {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("A___B.md"),
                    title: "B".to_owned(),
                    revision: FileFingerprint::from_text(""),
                    modified_at: None,
                    parent_page_id: Some(PageId::new(["A"]).unwrap()),
                    child_page_count: 0,
                },
                PageSummary {
                    page_id: PageId::new(["C"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("C.md"),
                    title: "C".to_owned(),
                    revision: FileFingerprint::from_text(""),
                    modified_at: None,
                    parent_page_id: None,
                    child_page_count: 0,
                },
            ]
        );
    }

    #[test]
    fn page_detail_returns_minimal_page_counts() {
        let text = "- parent [[B]]\n\t- child #Missing\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let detail = read_api.page_detail(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(detail.summary.title, "A");
        assert_eq!(detail.summary.revision, FileFingerprint::from_text(text));
        assert!(detail.incoming_refs.is_empty());
        assert_eq!(detail.outgoing_ref_count, 2);
        assert_eq!(detail.outgoing_refs.len(), 2);
    }

    #[test]
    fn incoming_refs_return_normalized_source_anchors() {
        let text = "- parent [[B]]\n\t- child\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let incoming_refs = read_api
            .page_incoming_refs(&PageId::new(["B"]).unwrap())
            .unwrap();

        assert_eq!(incoming_refs.len(), 1);
        assert_eq!(
            incoming_refs[0],
            IncomingPageRefSnapshot {
                target_page_id: PageId::new(["B"]).unwrap(),
                source_page_id: PageId::new(["A"]).unwrap(),
                source_page_fingerprint: FileFingerprint::from_text(text),
                source_block_span: SourceSpan::unchecked(0, text.len()),
                ref_span: SourceSpan::unchecked(9, 14),
            }
        );
    }

    #[test]
    fn incoming_refs_require_existing_target_pages() {
        let text = "- [[Missing]]\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        assert_eq!(
            read_api
                .page_incoming_refs(&PageId::new(["Missing"]).unwrap())
                .unwrap_err(),
            CoreError::MissingPage
        );
    }

    #[test]
    fn page_outgoing_refs_and_missing_target_queries_are_available() {
        let text = "- [[B]] and [[Missing]]\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let outgoing_refs = read_api
            .page_outgoing_refs(&PageId::new(["A"]).unwrap())
            .unwrap();
        assert_eq!(outgoing_refs.len(), 2);
        assert!(
            outgoing_refs
                .iter()
                .any(|outgoing_ref| outgoing_ref.target_exists)
        );
        assert!(
            outgoing_refs
                .iter()
                .any(|outgoing_ref| !outgoing_ref.target_exists)
        );

        assert_eq!(
            read_api
                .pages_with_missing_targets()
                .into_iter()
                .map(|summary| summary.page_id)
                .collect::<Vec<_>>(),
            vec![PageId::new(["A"]).unwrap()]
        );
        assert_eq!(read_api.all_pages_paginated(1, 1).len(), 1);
    }

    #[test]
    fn linked_refs_return_editable_block_subtrees_with_exact_highlight() {
        let text = "- parent [[B]]\n\t- child\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let linked_refs = read_api
            .page_linked_refs(&PageId::new(["B"]).unwrap())
            .unwrap();

        assert_eq!(linked_refs.len(), 1);
        assert_eq!(linked_refs[0].source_page_id, PageId::new(["A"]).unwrap());
        assert_eq!(linked_refs[0].block.content, "parent [[B]]");
        assert_eq!(linked_refs[0].block.markdown, text);
        assert_eq!(linked_refs[0].block.children.len(), 1);
        assert_eq!(
            linked_refs[0].block_content_highlight,
            Some(RefHighlightSnapshot {
                prefix: "parent ".to_owned(),
                highlight: "[[B]]".to_owned(),
                suffix: "".to_owned(),
            })
        );
    }

    #[test]
    fn block_snapshot_refreshes_current_block_even_from_stale_handle() {
        let text = "- body [[B]]\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let linked_ref = read_api
            .page_linked_refs(&PageId::new(["B"]).unwrap())
            .unwrap()
            .remove(0);
        let stale_handle = BlockHandle {
            source_page_id: linked_ref.block.handle.source_page_id,
            source_page_revision: FileFingerprint::from_text("stale"),
            block_span: linked_ref.block.handle.block_span,
        };

        let block = read_api.block_snapshot(&stale_handle).unwrap();
        assert_eq!(block.content, "body [[B]]");
        assert_eq!(
            block.handle.source_page_revision,
            FileFingerprint::from_text(text)
        );
    }

    #[test]
    fn search_pages_ranks_title_then_page_id_then_content_and_applies_limit() {
        let cache = WorkspaceCache::from_pages([
            parsed_page(&["Alpha"], "misc\n"),
            parsed_page(&["Beta"], "Alpha appears in content\n"),
            stream_page("alpha_stream", "2026_05_08", "other\n"),
        ]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let results = read_api.search_pages("alpha", 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].page_id, PageId::new(["Alpha"]).unwrap());
        assert_eq!(results[0].matched_field, SearchMatchField::Title);
        assert_eq!(
            results[1].page_id,
            PageId::stream(
                crate::PageName::new("alpha_stream").unwrap(),
                crate::PageName::new("2026_05_08").unwrap()
            )
            .unwrap()
        );
        assert_eq!(results[1].matched_field, SearchMatchField::PageId);
    }

    #[test]
    fn search_pages_is_case_insensitive_and_returns_content_snippets() {
        let cache = WorkspaceCache::from_pages([stream_page(
            "journal",
            "2026_05_07",
            "Line one\nNeedle inside content body for snippet coverage.\nLine three",
        )]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        let results = read_api.search_pages("nEeDlE", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_field, SearchMatchField::Content);
        let snippet = results[0].snippet.as_ref().unwrap();
        assert!(
            snippet
                .to_lowercase()
                .contains("needle inside content body")
        );
        assert!(!snippet.contains('\n'));
    }

    #[test]
    fn search_pages_returns_empty_for_blank_queries() {
        let cache = WorkspaceCache::from_pages([parsed_page(&["Alpha"], "")]);
        let read_api = WorkspaceReadApi::new(&cache, &|_| None);

        assert!(read_api.search_pages("   ", 10).is_empty());
        assert!(read_api.search_pages("Alpha", 0).is_empty());
    }
}

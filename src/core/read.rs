use std::path::PathBuf;

use super::{
    Block, BlockKind, CoreError, FileFingerprint, Page, PageId, PageLocation, PlaintextKind,
    SourceSpan, WorkspaceCache,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummary {
    pub page_id: PageId,
    pub location: PageLocation,
    pub workspace_path: PathBuf,
    pub title: String,
    pub revision: FileFingerprint,
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
pub struct BlockSnapshot {
    pub kind: BlockSnapshotKind,
    pub block_span: SourceSpan,
    pub content_span: SourceSpan,
    pub content: String,
    pub children: Vec<BlockSnapshot>,
    pub outgoing_refs: Vec<OutgoingPageRefSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBlocksSnapshot {
    pub page_id: PageId,
    pub revision: FileFingerprint,
    pub blocks: Vec<BlockSnapshot>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSnapshotKind {
    Outliner,
    ExplicitPlaintext,
    ImplicitPlaintext,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceReadApi<'a> {
    cache: &'a WorkspaceCache,
}

impl<'a> WorkspaceReadApi<'a> {
    pub fn new(cache: &'a WorkspaceCache) -> Self {
        Self { cache }
    }

    pub fn all_pages(&self) -> Vec<PageSummary> {
        self.cache.pages().values().map(page_summary).collect()
    }

    pub fn page_summary(&self, page_id: &PageId) -> Result<PageSummary, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(page_summary(page))
    }

    pub fn page_detail(&self, page_id: &PageId) -> Result<PageDetail, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(PageDetail {
            summary: page_summary(page),
            incoming_refs: incoming_ref_snapshots(page_id, page),
            outgoing_refs: outgoing_ref_snapshots(self.cache, page),
            outgoing_ref_count: page.outgoing_refs().count(),
        })
    }

    pub fn child_pages(&self, page_id: &PageId) -> Result<Vec<PageSummary>, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        page.child_page_ids
            .iter()
            .map(|child_page_id| {
                self.cache
                    .page(child_page_id)
                    .map(page_summary)
                    .ok_or(CoreError::MissingPage)
            })
            .collect()
    }

    pub fn page_blocks(&self, page_id: &PageId) -> Result<PageBlocksSnapshot, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        Ok(PageBlocksSnapshot {
            page_id: page.page_id.clone(),
            revision: page.fingerprint,
            blocks: page
                .blocks
                .iter()
                .map(|block| block_snapshot(self.cache, page, block))
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn page_incoming_refs(
        &self,
        target_page_id: &PageId,
    ) -> Result<Vec<IncomingPageRefSnapshot>, CoreError> {
        let target_page = self
            .cache
            .page(target_page_id)
            .ok_or(CoreError::MissingPage)?;
        Ok(incoming_ref_snapshots(target_page_id, target_page))
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

    pub fn pages_with_missing_targets(&self) -> Vec<PageSummary> {
        self.cache
            .pages()
            .values()
            .filter(|page| page.outgoing_refs().any(|outgoing_ref| self.cache.page(&outgoing_ref.target_page_id).is_none()))
            .map(page_summary)
            .collect()
    }

    pub fn all_pages_paginated(&self, offset: usize, limit: usize) -> Vec<PageSummary> {
        self.cache
            .pages()
            .values()
            .skip(offset)
            .take(limit)
            .map(page_summary)
            .collect()
    }
}

fn incoming_ref_snapshots(
    target_page_id: &PageId,
    target_page: &Page,
) -> Vec<IncomingPageRefSnapshot> {
    target_page
        .incoming_refs
        .iter()
        .map(|incoming_ref| IncomingPageRefSnapshot {
            target_page_id: target_page_id.clone(),
            source_page_id: incoming_ref.source_page_id.clone(),
            source_page_fingerprint: incoming_ref.source_page_fingerprint,
            source_block_span: incoming_ref.source_block_span,
            ref_span: incoming_ref.ref_span,
        })
        .collect()
}

fn page_summary(page: &Page) -> PageSummary {
    PageSummary {
        page_id: page.page_id.clone(),
        location: page.location.clone(),
        workspace_path: page.workspace_path.clone(),
        title: page.title.clone(),
        revision: page.fingerprint,
        parent_page_id: page.parent_page_id(),
        child_page_count: page.child_page_ids.len(),
    }
}

fn outgoing_ref_snapshots(cache: &WorkspaceCache, source_page: &Page) -> Vec<OutgoingPageRefSnapshot> {
    source_page
        .outgoing_refs()
        .map(|outgoing_ref| OutgoingPageRefSnapshot {
            target_page_id: outgoing_ref.target_page_id.clone(),
            ref_span: outgoing_ref.ref_span,
            target_exists: cache.page(&outgoing_ref.target_page_id).is_some(),
        })
        .collect()
}

fn block_snapshot(
    cache: &WorkspaceCache,
    source_page: &Page,
    block: &Block,
) -> Result<BlockSnapshot, CoreError> {
    Ok(BlockSnapshot {
        kind: block_snapshot_kind(block.kind),
        block_span: block.block_span,
        content_span: block.content_span,
        content: resolved_block_content(source_page, block)?,
        children: block
            .children
            .iter()
            .map(|child| block_snapshot(cache, source_page, child))
            .collect::<Result<_, _>>()?,
        outgoing_refs: block
            .outgoing_refs
            .iter()
            .map(|outgoing_ref| OutgoingPageRefSnapshot {
                target_page_id: outgoing_ref.target_page_id.clone(),
                ref_span: outgoing_ref.ref_span,
                target_exists: cache.page(&outgoing_ref.target_page_id).is_some(),
            })
            .collect(),
    })
}

fn block_snapshot_kind(kind: BlockKind) -> BlockSnapshotKind {
    match kind {
        BlockKind::Outliner => BlockSnapshotKind::Outliner,
        BlockKind::Plaintext(PlaintextKind::Explicit) => BlockSnapshotKind::ExplicitPlaintext,
        BlockKind::Plaintext(PlaintextKind::Implicit) => BlockSnapshotKind::ImplicitPlaintext,
    }
}

fn resolved_block_content(source_page: &Page, block: &Block) -> Result<String, CoreError> {
    let content = block.content_span.slice(&source_page.text)?;
    Ok(content
        .strip_suffix("\r\n")
        .or_else(|| content.strip_suffix('\n'))
        .unwrap_or(content)
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Page, parse_blocks};

    fn parsed_page(id: &[&str], text: &str) -> Page {
        Page::new(PageId::new(id.iter().copied()).unwrap(), text)
            .with_blocks(parse_blocks(text).unwrap())
    }

    #[test]
    fn lists_pages_as_stable_summaries() {
        let cache = WorkspaceCache::from_pages([
            parsed_page(&["A"], ""),
            parsed_page(&["A", "B"], ""),
            parsed_page(&["C"], ""),
        ]);
        let read_api = WorkspaceReadApi::new(&cache);

        assert_eq!(
            read_api.all_pages(),
            vec![
                PageSummary {
                    page_id: PageId::new(["A"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("A.md"),
                    title: "A".to_owned(),
                    revision: FileFingerprint::from_text(""),
                    parent_page_id: None,
                    child_page_count: 1,
                },
                PageSummary {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("A___B.md"),
                    title: "B".to_owned(),
                    revision: FileFingerprint::from_text(""),
                    parent_page_id: Some(PageId::new(["A"]).unwrap()),
                    child_page_count: 0,
                },
                PageSummary {
                    page_id: PageId::new(["C"]).unwrap(),
                    location: crate::PageLocation::Pages,
                    workspace_path: std::path::PathBuf::from("pages").join("C.md"),
                    title: "C".to_owned(),
                    revision: FileFingerprint::from_text(""),
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
        let read_api = WorkspaceReadApi::new(&cache);

        let detail = read_api.page_detail(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(detail.summary.title, "A");
        assert_eq!(detail.summary.revision, FileFingerprint::from_text(text));
        assert!(detail.incoming_refs.is_empty());
        assert_eq!(detail.outgoing_ref_count, 2);
        assert_eq!(detail.outgoing_refs.len(), 2);
    }

    #[test]
    fn page_blocks_keep_unresolved_outgoing_refs_visible() {
        let text = "- [[Missing]]\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache);

        let blocks = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(blocks.blocks[0].outgoing_refs.len(), 1);
        assert_eq!(
            blocks.blocks[0].outgoing_refs[0],
            OutgoingPageRefSnapshot {
                target_page_id: PageId::new(["Missing"]).unwrap(),
                ref_span: SourceSpan::unchecked(2, 13),
                target_exists: false,
            }
        );
    }

    #[test]
    fn page_blocks_expose_source_spans_without_revision_handles() {
        let text = "- current\n\t- child\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache);
        let blocks = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(blocks.page_id, PageId::new(["A"]).unwrap());
        assert_eq!(blocks.revision, FileFingerprint::from_text(text));
        assert_eq!(blocks.blocks[0].block_span, SourceSpan::unchecked(0, text.len()));
        assert_eq!(blocks.blocks[0].content_span, SourceSpan::unchecked(2, 10));
        assert_eq!(blocks.blocks[0].children[0].content, "child");
    }

    #[test]
    fn incoming_refs_return_normalized_source_anchors() {
        let text = "- parent [[B]]\n\t- child\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache);

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
        let read_api = WorkspaceReadApi::new(&cache);

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
        let read_api = WorkspaceReadApi::new(&cache);

        let outgoing_refs = read_api
            .page_outgoing_refs(&PageId::new(["A"]).unwrap())
            .unwrap();
        assert_eq!(outgoing_refs.len(), 2);
        assert!(outgoing_refs.iter().any(|outgoing_ref| outgoing_ref.target_exists));
        assert!(outgoing_refs
            .iter()
            .any(|outgoing_ref| !outgoing_ref.target_exists));

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
}

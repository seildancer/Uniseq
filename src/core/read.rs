use super::{
    Block, BlockHandle, BlockKind, CoreError, Page, PageId, PlaintextKind, SourceSpan,
    WorkspaceCache,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummary {
    pub page_id: PageId,
    pub title: String,
    pub parent_page_id: Option<PageId>,
    pub child_page_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDetail {
    pub summary: PageSummary,
    pub child_pages: Vec<PageSummary>,
    pub root_blocks: Vec<BlockSnapshot>,
    pub linked_ref_count: usize,
    pub outgoing_ref_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSnapshot {
    pub handle: BlockHandle,
    pub kind: BlockSnapshotKind,
    pub content: String,
    pub children: Vec<BlockSnapshot>,
    pub outgoing_refs: Vec<OutgoingPageRefSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRefEntry {
    pub target_page_id: PageId,
    pub source_page: PageSummary,
    pub source_block: BlockSnapshot,
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
            child_pages: page
                .child_page_ids
                .iter()
                .map(|child_page_id| {
                    self.cache
                        .page(child_page_id)
                        .expect("workspace cache hierarchy only points at existing pages")
                })
                .map(page_summary)
                .collect(),
            root_blocks: page
                .blocks
                .iter()
                .map(|block| block_snapshot(self.cache, page, block))
                .collect::<Result<_, _>>()?,
            linked_ref_count: page.incoming_refs.len(),
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

    pub fn page_blocks(&self, page_id: &PageId) -> Result<Vec<BlockSnapshot>, CoreError> {
        let page = self.cache.page(page_id).ok_or(CoreError::MissingPage)?;
        page.blocks
            .iter()
            .map(|block| block_snapshot(self.cache, page, block))
            .collect()
    }

    pub fn block_by_handle(&self, handle: &BlockHandle) -> Result<BlockSnapshot, CoreError> {
        let page = self
            .cache
            .page(handle.source_page_id())
            .ok_or(CoreError::MissingPage)?;

        if page.fingerprint != handle.source_page_fingerprint() {
            return Err(CoreError::StalePageRevision);
        }

        let block = page
            .find_block_by_span(handle.block_span())
            .ok_or(CoreError::MissingBlock)?;

        block_snapshot(self.cache, page, block)
    }

    pub fn linked_refs(&self, target_page_id: &PageId) -> Result<Vec<LinkedRefEntry>, CoreError> {
        let target_page = self
            .cache
            .page(target_page_id)
            .ok_or(CoreError::MissingPage)?;

        target_page
            .incoming_refs
            .iter()
            .map(|incoming_ref| {
                let source_page = self
                    .cache
                    .page(&incoming_ref.source_page_id)
                    .ok_or(CoreError::MissingPage)?;

                if source_page.fingerprint != incoming_ref.source_page_fingerprint {
                    return Err(CoreError::StalePageRevision);
                }

                let source_block = source_page
                    .find_block_by_span(incoming_ref.source_block_span)
                    .ok_or(CoreError::MissingBlock)?;

                Ok(LinkedRefEntry {
                    target_page_id: target_page_id.clone(),
                    source_page: page_summary(source_page),
                    source_block: block_snapshot(self.cache, source_page, source_block)?,
                    ref_span: incoming_ref.ref_span,
                })
            })
            .collect()
    }
}

fn page_summary(page: &Page) -> PageSummary {
    PageSummary {
        page_id: page.page_id.clone(),
        title: page.title.clone(),
        parent_page_id: page.page_id.parent(),
        child_page_count: page.child_page_ids.len(),
    }
}

fn block_snapshot(
    cache: &WorkspaceCache,
    source_page: &Page,
    block: &Block,
) -> Result<BlockSnapshot, CoreError> {
    Ok(BlockSnapshot {
        handle: BlockHandle::new(
            source_page.page_id.clone(),
            source_page.fingerprint,
            block.block_span,
        ),
        kind: block_snapshot_kind(block.kind),
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
                    title: "A".to_owned(),
                    parent_page_id: None,
                    child_page_count: 1,
                },
                PageSummary {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                    title: "B".to_owned(),
                    parent_page_id: Some(PageId::new(["A"]).unwrap()),
                    child_page_count: 0,
                },
                PageSummary {
                    page_id: PageId::new(["C"]).unwrap(),
                    title: "C".to_owned(),
                    parent_page_id: None,
                    child_page_count: 0,
                },
            ]
        );
    }

    #[test]
    fn page_detail_returns_resolved_blocks_and_reference_counts() {
        let text = "- parent [[B]]\n\t- child #Missing\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache);

        let detail = read_api.page_detail(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(detail.summary.title, "A");
        assert_eq!(detail.linked_ref_count, 0);
        assert_eq!(detail.outgoing_ref_count, 2);
        assert_eq!(detail.root_blocks.len(), 1);
        assert_eq!(detail.root_blocks[0].content, "parent [[B]]");
        assert_eq!(detail.root_blocks[0].children[0].content, "child #Missing");
    }

    #[test]
    fn page_blocks_keep_unresolved_outgoing_refs_visible() {
        let text = "- [[Missing]]\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache);

        let blocks = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();

        assert_eq!(blocks[0].outgoing_refs.len(), 1);
        assert_eq!(
            blocks[0].outgoing_refs[0],
            OutgoingPageRefSnapshot {
                target_page_id: PageId::new(["Missing"]).unwrap(),
                ref_span: SourceSpan::unchecked(2, 13),
                target_exists: false,
            }
        );
    }

    #[test]
    fn block_handle_round_trips_and_detects_stale_pages() {
        let text = "- current\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache);
        let handle = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap()[0]
            .handle
            .clone();

        let block = read_api.block_by_handle(&handle).unwrap();
        assert_eq!(block.content, "current");

        let stale_cache = WorkspaceCache::from_pages([parsed_page(&["A"], "- updated\n")]);
        let stale_read_api = WorkspaceReadApi::new(&stale_cache);

        assert_eq!(
            stale_read_api.block_by_handle(&handle).unwrap_err(),
            CoreError::StalePageRevision
        );
    }

    #[test]
    fn linked_refs_return_editable_source_block_snapshots() {
        let text = "- parent [[B]]\n\t- child\n";
        let cache =
            WorkspaceCache::from_pages([parsed_page(&["A"], text), parsed_page(&["B"], "")]);
        let read_api = WorkspaceReadApi::new(&cache);

        let from_page = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();
        let linked_refs = read_api.linked_refs(&PageId::new(["B"]).unwrap()).unwrap();

        assert_eq!(linked_refs.len(), 1);
        assert_eq!(linked_refs[0].target_page_id, PageId::new(["B"]).unwrap());
        assert_eq!(
            linked_refs[0].source_page.page_id,
            PageId::new(["A"]).unwrap()
        );
        assert_eq!(linked_refs[0].source_block.content, "parent [[B]]");
        assert_eq!(linked_refs[0].source_block.children[0].content, "child");
        assert_eq!(linked_refs[0].source_block.handle, from_page[0].handle);
        assert_eq!(linked_refs[0].ref_span.slice(text).unwrap(), "[[B]]");
    }

    #[test]
    fn linked_refs_require_existing_target_pages() {
        let text = "- [[Missing]]\n";
        let cache = WorkspaceCache::from_pages([parsed_page(&["A"], text)]);
        let read_api = WorkspaceReadApi::new(&cache);

        assert_eq!(
            read_api
                .linked_refs(&PageId::new(["Missing"]).unwrap())
                .unwrap_err(),
            CoreError::MissingPage
        );
    }
}

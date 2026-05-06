use std::collections::{BTreeMap, BTreeSet};

use super::{
    Block, BlockHandle, CoreError, FileFingerprint, IncomingRef, Page, PageId, parse_blocks,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSubtreeEdit {
    pub block_handle: BlockHandle,
    pub replacement_markdown: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorkspaceCache {
    pages: BTreeMap<PageId, Page>,
}

impl WorkspaceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_pages<I>(pages: I) -> Self
    where
        I: IntoIterator<Item = Page>,
    {
        let mut cache = Self {
            pages: pages
                .into_iter()
                .map(|page| (page.page_id.clone(), page))
                .collect(),
        };
        cache.rebuild_hierarchy();
        cache.rebuild_all_incoming_refs();
        cache
    }

    pub fn pages(&self) -> &BTreeMap<PageId, Page> {
        &self.pages
    }

    pub fn page(&self, page_id: &PageId) -> Option<&Page> {
        self.pages.get(page_id)
    }

    pub fn page_mut(&mut self, page_id: &PageId) -> Option<&mut Page> {
        self.pages.get_mut(page_id)
    }

    pub fn upsert_page(&mut self, page: Page) {
        let page_id = page.page_id.clone();
        self.pages.insert(page_id, page);
        self.rebuild_hierarchy();
        self.rebuild_all_incoming_refs();
    }

    pub fn remove_page(&mut self, page_id: &PageId) -> Option<Page> {
        let removed = self.pages.remove(page_id)?;
        self.rebuild_hierarchy();
        self.rebuild_all_incoming_refs();
        Some(removed)
    }

    pub fn reparse_and_upsert_page_markdown(
        &mut self,
        page_id: &PageId,
        text: impl Into<String>,
    ) -> Result<(), CoreError> {
        let text = text.into();
        let blocks = parse_blocks(&text)?;
        self.upsert_page(Page::new(page_id.clone(), text).with_blocks(blocks));
        Ok(())
    }

    pub fn missing_parent_page_ids(&self) -> Vec<PageId> {
        let existing = self.pages.keys().collect::<BTreeSet<_>>();
        let mut missing = BTreeSet::new();

        for page_id in self.pages.keys() {
            for ancestor in page_id.ancestors() {
                if !existing.contains(&ancestor) {
                    missing.insert(ancestor);
                }
            }
        }

        missing.into_iter().collect()
    }

    fn rebuild_hierarchy(&mut self) {
        for page in self.pages.values_mut() {
            page.child_page_ids.clear();
        }

        let page_ids = self.pages.keys().cloned().collect::<Vec<_>>();
        for page_id in page_ids {
            if let Some(parent_id) = page_id.parent() {
                if let Some(parent) = self.pages.get_mut(&parent_id) {
                    parent.child_page_ids.push(page_id);
                }
            }
        }

        for page in self.pages.values_mut() {
            page.child_page_ids.sort();
        }
    }

    fn rebuild_all_incoming_refs(&mut self) {
        let source_page_ids = self.pages.keys().cloned().collect::<Vec<_>>();
        for page in self.pages.values_mut() {
            page.incoming_refs.clear();
        }

        for source_page_id in source_page_ids {
            self.insert_incoming_refs_from_source(&source_page_id);
        }
    }

    fn insert_incoming_refs_from_source(&mut self, source_page_id: &PageId) {
        let Some(source_page) = self.pages.get(source_page_id) else {
            return;
        };

        let source_fingerprint = source_page.fingerprint;
        let incoming_refs = source_page
            .blocks
            .iter()
            .flat_map(|block| incoming_refs_from_block(source_page_id, source_fingerprint, block))
            .collect::<Vec<_>>();

        for incoming_ref in incoming_refs {
            if let Some(target_page) = self.pages.get_mut(&incoming_ref.target_page_id) {
                target_page.incoming_refs.push(incoming_ref.incoming_ref);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct IncomingRefWithTarget {
    target_page_id: PageId,
    incoming_ref: IncomingRef,
}

fn incoming_refs_from_block(
    source_page_id: &PageId,
    source_fingerprint: FileFingerprint,
    block: &Block,
) -> Vec<IncomingRefWithTarget> {
    let mut refs = block
        .outgoing_refs
        .iter()
        .map(|outgoing| IncomingRefWithTarget {
            target_page_id: outgoing.target_page_id.clone(),
            incoming_ref: IncomingRef::new(
                source_page_id.clone(),
                source_fingerprint,
                block.block_span,
                outgoing.ref_span,
            ),
        })
        .collect::<Vec<_>>();

    refs.extend(
        block
            .children
            .iter()
            .flat_map(|child| incoming_refs_from_block(source_page_id, source_fingerprint, child)),
    );

    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockKind, PageRefOccurrence, PlaintextKind, SourceSpan, parse_blocks};

    fn page(id: &[&str], text: &str, blocks: Vec<Block>) -> Page {
        Page::new(PageId::new(id.iter().copied()).unwrap(), text).with_blocks(blocks)
    }

    fn ref_block(block_span: SourceSpan, target: &[&str], ref_span: SourceSpan) -> Block {
        Block::new(
            BlockKind::Outliner,
            block_span,
            block_span,
            Vec::new(),
            vec![PageRefOccurrence::new(
                PageId::new(target.iter().copied()).unwrap(),
                ref_span,
            )],
        )
    }

    #[test]
    fn records_child_pages_from_page_hierarchy() {
        let mut cache = WorkspaceCache::new();
        cache.upsert_page(page(&["A"], "", Vec::new()));
        cache.upsert_page(page(&["A", "B"], "", Vec::new()));

        let a = cache.page(&PageId::new(["A"]).unwrap()).unwrap();
        assert_eq!(a.child_page_ids, vec![PageId::new(["A", "B"]).unwrap()]);
    }

    #[test]
    fn reports_missing_parent_pages() {
        let mut cache = WorkspaceCache::new();
        cache.upsert_page(page(&["A", "B", "C"], "", Vec::new()));

        assert_eq!(
            cache.missing_parent_page_ids(),
            vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap()
            ]
        );
    }

    #[test]
    fn incoming_refs_point_to_source_owned_blocks() {
        let mut cache = WorkspaceCache::new();
        let source_block = ref_block(
            SourceSpan::unchecked(0, 12),
            &["B"],
            SourceSpan::unchecked(4, 9),
        );

        cache.upsert_page(page(&["A"], "- [[B]]\n", vec![source_block]));
        cache.upsert_page(page(&["B"], "", Vec::new()));

        let b = cache.page(&PageId::new(["B"]).unwrap()).unwrap();
        assert_eq!(b.incoming_refs.len(), 1);

        let incoming = &b.incoming_refs[0];
        assert_eq!(incoming.source_page_id, PageId::new(["A"]).unwrap());
        assert_eq!(incoming.source_block_span, SourceSpan::unchecked(0, 12));

        let source_page = cache.page(&incoming.source_page_id).unwrap();
        assert!(
            source_page
                .find_block_by_span(incoming.source_block_span)
                .is_some()
        );
    }

    #[test]
    fn reparse_and_upsert_page_markdown_reparses_and_rebuilds_incoming_refs() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(
            Page::new(source_page_id.clone(), "- [[B]]\n").with_blocks(vec![ref_block(
                SourceSpan::unchecked(0, 8),
                &["B"],
                SourceSpan::unchecked(2, 7),
            )]),
        );
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache.upsert_page(page(&["C"], "", Vec::new()));

        assert_eq!(
            cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
        assert_eq!(
            cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );

        cache
            .reparse_and_upsert_page_markdown(&source_page_id, "- [[C]]\n")
            .unwrap();

        assert_eq!(
            cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );
        assert_eq!(
            cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn reparse_and_upsert_page_markdown_rebuilds_incoming_refs_for_whole_page_updates() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();

        cache.upsert_page(page(&["A"], "", Vec::new()));
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache
            .reparse_and_upsert_page_markdown(&source_page_id, "- [[B]]\n")
            .unwrap();

        assert_eq!(
            cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn parsed_blocks_feed_incoming_ref_indexing() {
        let mut cache = WorkspaceCache::new();
        let text = "- [[B]]\n\t- #C\n";

        cache.upsert_page(
            Page::new(PageId::new(["A"]).unwrap(), text).with_blocks(parse_blocks(text).unwrap()),
        );
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache.upsert_page(page(&["C"], "", Vec::new()));

        assert_eq!(
            cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
        assert_eq!(
            cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn reparse_and_upsert_page_markdown_accepts_real_parser_output() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();

        cache.upsert_page(
            Page::new(source_page_id.clone(), "- [[B]]\n")
                .with_blocks(parse_blocks("- [[B]]\n").unwrap()),
        );
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache.upsert_page(page(&["C"], "", Vec::new()));

        cache
            .reparse_and_upsert_page_markdown(&source_page_id, "- [[C]]\n")
            .unwrap();

        assert_eq!(
            cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );
        assert_eq!(
            cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn reparse_and_upsert_page_markdown_can_replace_page_with_empty_markdown() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(
            Page::new(source_page_id.clone(), "- old\n").with_blocks(vec![Block::leaf(
                BlockKind::Plaintext(PlaintextKind::Implicit),
                SourceSpan::unchecked(0, 6),
                SourceSpan::unchecked(2, 5),
            )]),
        );

        cache
            .reparse_and_upsert_page_markdown(&source_page_id, "")
            .unwrap();
        let page = cache.page(&source_page_id).unwrap();
        assert_eq!(page.text, "");
        assert!(page.blocks.is_empty());
    }
}

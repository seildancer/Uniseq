use std::collections::{BTreeMap, BTreeSet};

use super::{Block, CoreError, FileFingerprint, IncomingRef, Page, PageId, SourceSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalUpdate {
    pub page_id: PageId,
    pub expected_fingerprint: FileFingerprint,
    pub replaced_block_span: SourceSpan,
    pub replacement_blocks: Vec<Block>,
    pub new_fingerprint: FileFingerprint,
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

    pub fn replace_page_blocks(
        &mut self,
        page_id: &PageId,
        blocks: Vec<Block>,
        new_fingerprint: FileFingerprint,
    ) -> Result<(), CoreError> {
        let page = self.pages.get_mut(page_id).ok_or(CoreError::MissingPage)?;
        page.set_blocks(blocks, new_fingerprint);
        self.rebuild_incoming_refs_after_source_change(page_id);
        Ok(())
    }

    pub fn apply_incremental_update(&mut self, update: IncrementalUpdate) -> Result<(), CoreError> {
        let page = self
            .pages
            .get_mut(&update.page_id)
            .ok_or(CoreError::MissingPage)?;

        if page.fingerprint != update.expected_fingerprint {
            return Err(CoreError::StalePageRevision);
        }

        replace_blocks_by_span(
            &mut page.blocks,
            update.replaced_block_span,
            update.replacement_blocks,
        )
        .ok_or(CoreError::InvalidSpan(super::SpanError::OutOfBounds {
            span_end: update.replaced_block_span.end(),
            text_len: page.fingerprint.len_bytes(),
        }))?;

        page.fingerprint = update.new_fingerprint;
        self.rebuild_incoming_refs_after_source_change(&update.page_id);
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

    fn rebuild_incoming_refs_after_source_change(&mut self, source_page_id: &PageId) {
        for page in self.pages.values_mut() {
            page.incoming_refs
                .retain(|incoming| &incoming.source_page_id != source_page_id);
        }

        self.insert_incoming_refs_from_source(source_page_id);
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

fn replace_blocks_by_span(
    blocks: &mut Vec<Block>,
    replaced_block_span: SourceSpan,
    replacement_blocks: Vec<Block>,
) -> Option<()> {
    if let Some(index) = blocks
        .iter()
        .position(|block| block.block_span == replaced_block_span)
    {
        blocks.splice(index..=index, replacement_blocks);
        return Some(());
    }

    for block in blocks {
        if replace_blocks_by_span(
            &mut block.children,
            replaced_block_span,
            replacement_blocks.clone(),
        )
        .is_some()
        {
            return Some(());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageRefOccurrence, SpanError};

    fn page(id: &[&str], text: &str, blocks: Vec<Block>) -> Page {
        Page::new(
            PageId::new(id.iter().copied()).unwrap(),
            FileFingerprint::from_text(text),
        )
        .with_blocks(blocks)
    }

    fn ref_block(block_span: SourceSpan, target: &[&str], ref_span: SourceSpan) -> Block {
        Block::new(
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
    fn incremental_update_rewrites_only_refs_from_changed_subtree() {
        let mut cache = WorkspaceCache::new();
        let original_fingerprint = FileFingerprint::from_text("- [[B]]\n");
        let source_page_id = PageId::new(["A"]).unwrap();
        let old_block = ref_block(
            SourceSpan::unchecked(0, 8),
            &["B"],
            SourceSpan::unchecked(2, 7),
        );
        let new_block = ref_block(
            SourceSpan::unchecked(0, 8),
            &["C"],
            SourceSpan::unchecked(2, 7),
        );

        cache.upsert_page(
            Page::new(source_page_id.clone(), original_fingerprint).with_blocks(vec![old_block]),
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
            .apply_incremental_update(IncrementalUpdate {
                page_id: source_page_id,
                expected_fingerprint: original_fingerprint,
                replaced_block_span: SourceSpan::unchecked(0, 8),
                replacement_blocks: vec![new_block],
                new_fingerprint: FileFingerprint::from_text("- [[C]]\n"),
            })
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
    fn stale_incremental_update_is_rejected() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(page(
            &["A"],
            "- old\n",
            vec![Block::leaf(
                SourceSpan::unchecked(0, 6),
                SourceSpan::unchecked(2, 5),
            )],
        ));

        let result = cache.apply_incremental_update(IncrementalUpdate {
            page_id: source_page_id,
            expected_fingerprint: FileFingerprint::from_text("- stale\n"),
            replaced_block_span: SourceSpan::unchecked(0, 6),
            replacement_blocks: Vec::new(),
            new_fingerprint: FileFingerprint::from_text(""),
        });

        assert_eq!(result.unwrap_err(), CoreError::StalePageRevision);
    }

    #[test]
    fn whole_page_replacement_rebuilds_incoming_refs_for_fallbacks() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();

        cache.upsert_page(page(&["A"], "", Vec::new()));
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache
            .replace_page_blocks(
                &source_page_id,
                vec![ref_block(
                    SourceSpan::unchecked(0, 8),
                    &["B"],
                    SourceSpan::unchecked(2, 7),
                )],
                FileFingerprint::from_text("- [[B]]\n"),
            )
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
    fn missing_block_span_is_reported_as_invalid_span() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        let fingerprint = FileFingerprint::from_text("- old\n");
        cache.upsert_page(
            Page::new(source_page_id.clone(), fingerprint).with_blocks(vec![Block::leaf(
                SourceSpan::unchecked(0, 6),
                SourceSpan::unchecked(2, 5),
            )]),
        );

        let result = cache.apply_incremental_update(IncrementalUpdate {
            page_id: source_page_id,
            expected_fingerprint: fingerprint,
            replaced_block_span: SourceSpan::unchecked(20, 25),
            replacement_blocks: Vec::new(),
            new_fingerprint: FileFingerprint::from_text(""),
        });

        assert_eq!(
            result.unwrap_err(),
            CoreError::InvalidSpan(SpanError::OutOfBounds {
                span_end: 25,
                text_len: 6
            })
        );
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{Block, CoreError, FileFingerprint, IncomingRef, Page, PageId, parse_blocks};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorkspaceCache {
    pages: BTreeMap<PageId, Page>,
    page_ids_by_path: BTreeMap<std::path::PathBuf, PageId>,
    incoming_refs_by_target: BTreeMap<PageId, Vec<IncomingRef>>,
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
            page_ids_by_path: BTreeMap::new(),
            incoming_refs_by_target: BTreeMap::new(),
        };
        cache.rebuild_path_index();
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

    pub fn page_id_for_path(&self, workspace_path: impl AsRef<Path>) -> Option<&PageId> {
        self.page_ids_by_path.get(workspace_path.as_ref())
    }

    pub fn incoming_refs(&self, page_id: &PageId) -> &[IncomingRef] {
        self.incoming_refs_by_target
            .get(page_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn upsert_page(&mut self, mut page: Page) {
        let page_id = page.page_id.clone();
        self.remove_child_from_parent(&page_id);
        self.remove_incoming_refs_from_source(&page_id);
        if let Some(existing_page) = self.pages.get(&page_id) {
            self.page_ids_by_path.remove(&existing_page.workspace_path);
        }
        page.child_page_ids = self.child_page_ids_for(&page_id);
        self.page_ids_by_path
            .insert(page.workspace_path.clone(), page_id.clone());
        self.pages.insert(page_id.clone(), page);
        self.add_child_to_parent(page_id.clone());
        self.insert_incoming_refs_from_source(&page_id);
    }

    pub fn refresh_page_content(&mut self, mut page: Page) {
        let page_id = page.page_id.clone();
        let Some(existing_page) = self.pages.get(&page_id).cloned() else {
            self.upsert_page(page);
            return;
        };

        let old_target_page_ids = target_page_ids_from_page(&existing_page);
        if existing_page.workspace_path != page.workspace_path {
            self.page_ids_by_path.remove(&existing_page.workspace_path);
            self.page_ids_by_path
                .insert(page.workspace_path.clone(), page_id.clone());
        }
        page.child_page_ids = existing_page.child_page_ids;
        self.pages.insert(page_id.clone(), page);
        self.remove_incoming_refs_from_source_from_targets(&page_id, &old_target_page_ids);
        self.insert_incoming_refs_from_source(&page_id);
    }

    pub fn remove_page(&mut self, page_id: &PageId) -> Option<Page> {
        self.remove_child_from_parent(page_id);
        self.remove_incoming_refs_from_source(page_id);
        let removed = self.pages.remove(page_id)?;
        self.page_ids_by_path.remove(&removed.workspace_path);
        Some(removed)
    }

    pub fn reparse_and_upsert_page_markdown(
        &mut self,
        page_id: &PageId,
        text: impl Into<String>,
    ) -> Result<(), CoreError> {
        let text = text.into();
        let blocks = parse_blocks(&text)?;
        let location = self
            .page(page_id)
            .map(|page| page.location.clone())
            .unwrap_or(super::PageLocation::Pages);
        let page = Page::new_in_location(page_id.clone(), location, text)?.with_blocks(blocks);
        if self.page(page_id).is_some() {
            self.refresh_page_content(page);
        } else {
            self.upsert_page(page);
        }
        Ok(())
    }

    pub fn missing_parent_page_ids(&self) -> Vec<PageId> {
        let existing = self.pages.keys().collect::<BTreeSet<_>>();
        let mut missing = BTreeSet::new();

        for page in self.pages.values() {
            for ancestor in page.ancestor_page_ids() {
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

        let page_ids = self
            .pages
            .values()
            .map(|page| (page.page_id.clone(), page.parent_page_id()))
            .collect::<Vec<_>>();
        for (page_id, parent_page_id) in page_ids {
            if let Some(parent_id) = parent_page_id {
                if let Some(parent) = self.pages.get_mut(&parent_id) {
                    parent.child_page_ids.push(page_id);
                }
            }
        }

        for page in self.pages.values_mut() {
            page.child_page_ids.sort();
        }
    }

    fn rebuild_path_index(&mut self) {
        self.page_ids_by_path = self
            .pages
            .values()
            .map(|page| (page.workspace_path.clone(), page.page_id.clone()))
            .collect();
    }

    fn rebuild_all_incoming_refs(&mut self) {
        let source_page_ids = self.pages.keys().cloned().collect::<Vec<_>>();
        self.incoming_refs_by_target.clear();

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
            self.incoming_refs_by_target
                .entry(incoming_ref.target_page_id)
                .or_default()
                .push(incoming_ref.incoming_ref);
        }
    }

    fn child_page_ids_for(&self, parent_page_id: &PageId) -> Vec<PageId> {
        self.pages
            .values()
            .filter(|page| page.parent_page_id().as_ref() == Some(parent_page_id))
            .map(|page| page.page_id.clone())
            .collect()
    }

    fn add_child_to_parent(&mut self, page_id: PageId) {
        let Some(parent_id) = self.page(&page_id).and_then(Page::parent_page_id) else {
            return;
        };

        let Some(parent) = self.pages.get_mut(&parent_id) else {
            return;
        };

        if !parent.child_page_ids.contains(&page_id) {
            parent.child_page_ids.push(page_id);
            parent.child_page_ids.sort();
        }
    }

    fn remove_child_from_parent(&mut self, page_id: &PageId) {
        let Some(parent_id) = self.page(page_id).and_then(Page::parent_page_id) else {
            return;
        };

        if let Some(parent) = self.pages.get_mut(&parent_id) {
            parent.child_page_ids.retain(|child_id| child_id != page_id);
        }
    }

    fn remove_incoming_refs_from_source(&mut self, source_page_id: &PageId) {
        for incoming_refs in self.incoming_refs_by_target.values_mut() {
            incoming_refs.retain(|incoming_ref| &incoming_ref.source_page_id != source_page_id);
        }
        self.incoming_refs_by_target
            .retain(|_, incoming_refs| !incoming_refs.is_empty());
    }

    fn remove_incoming_refs_from_source_from_targets(
        &mut self,
        source_page_id: &PageId,
        target_page_ids: &BTreeSet<PageId>,
    ) {
        for target_page_id in target_page_ids {
            if let Some(incoming_refs) = self.incoming_refs_by_target.get_mut(target_page_id) {
                incoming_refs.retain(|incoming_ref| &incoming_ref.source_page_id != source_page_id);
            }
        }
        self.incoming_refs_by_target
            .retain(|_, incoming_refs| !incoming_refs.is_empty());
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

fn target_page_ids_from_page(page: &Page) -> BTreeSet<PageId> {
    page.outgoing_refs()
        .map(|outgoing_ref| outgoing_ref.target_page_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockKind, PageRefOccurrence, SourceSpan, parse_blocks};

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

        let incoming_refs = cache.incoming_refs(&PageId::new(["B"]).unwrap());
        assert_eq!(incoming_refs.len(), 1);

        let incoming = &incoming_refs[0];
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

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 1);
        assert_eq!(cache.incoming_refs(&PageId::new(["C"]).unwrap()).len(), 0);

        cache
            .reparse_and_upsert_page_markdown(&source_page_id, "- [[C]]\n")
            .unwrap();

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 0);
        assert_eq!(cache.incoming_refs(&PageId::new(["C"]).unwrap()).len(), 1);
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

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 1);
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

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 1);
        assert_eq!(cache.incoming_refs(&PageId::new(["C"]).unwrap()).len(), 1);
    }

    #[test]
    fn upsert_new_target_page_collects_existing_incoming_refs() {
        let mut cache = WorkspaceCache::new();
        cache.upsert_page(
            Page::new(PageId::new(["A"]).unwrap(), "- [[B]]\n")
                .with_blocks(parse_blocks("- [[B]]\n").unwrap()),
        );

        cache.upsert_page(page(&["B"], "", Vec::new()));

        let b_incoming_refs = cache.incoming_refs(&PageId::new(["B"]).unwrap());
        assert_eq!(b_incoming_refs.len(), 1);
        assert_eq!(
            b_incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
        );
    }

    #[test]
    fn upsert_existing_target_page_preserves_incoming_refs_from_other_pages() {
        let mut cache = WorkspaceCache::new();
        cache.upsert_page(page(&["B"], "- old\n", Vec::new()));
        cache.upsert_page(
            Page::new(PageId::new(["A"]).unwrap(), "- [[B]]\n")
                .with_blocks(parse_blocks("- [[B]]\n").unwrap()),
        );

        cache.upsert_page(
            Page::new(PageId::new(["B"]).unwrap(), "- new\n")
                .with_blocks(parse_blocks("- new\n").unwrap()),
        );

        let b = cache.page(&PageId::new(["B"]).unwrap()).unwrap();
        assert_eq!(b.text, "- new\n");
        let b_incoming_refs = cache.incoming_refs(&PageId::new(["B"]).unwrap());
        assert_eq!(b_incoming_refs.len(), 1);
        assert_eq!(
            b_incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
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

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 0);
        assert_eq!(cache.incoming_refs(&PageId::new(["C"]).unwrap()).len(), 1);
    }

    #[test]
    fn reparse_and_upsert_page_markdown_can_replace_page_with_empty_markdown() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(
            Page::new(source_page_id.clone(), "- old\n").with_blocks(vec![Block::leaf(
                BlockKind::Plaintext,
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

    #[test]
    fn refresh_page_content_preserves_incoming_refs_from_other_pages() {
        let mut cache = WorkspaceCache::new();
        let target_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(page(&["A"], "- old\n", Vec::new()));
        cache.upsert_page(
            Page::new(PageId::new(["X"]).unwrap(), "- [[A]]\n")
                .with_blocks(parse_blocks("- [[A]]\n").unwrap()),
        );

        cache.refresh_page_content(
            Page::new(target_page_id.clone(), "- new\n")
                .with_blocks(parse_blocks("- new\n").unwrap()),
        );

        let page = cache.page(&target_page_id).unwrap();
        assert_eq!(page.text, "- new\n");
        let incoming_refs = cache.incoming_refs(&target_page_id);
        assert_eq!(incoming_refs.len(), 1);
        assert_eq!(incoming_refs[0].source_page_id, PageId::new(["X"]).unwrap());
    }

    #[test]
    fn refresh_page_content_preserves_child_page_ids() {
        let mut cache = WorkspaceCache::new();
        let parent_page_id = PageId::new(["A"]).unwrap();
        let child_page_id = PageId::new(["A", "B"]).unwrap();
        cache.upsert_page(page(&["A"], "- old\n", Vec::new()));
        cache.upsert_page(page(&["A", "B"], "", Vec::new()));

        cache.refresh_page_content(
            Page::new(parent_page_id.clone(), "- new\n")
                .with_blocks(parse_blocks("- new\n").unwrap()),
        );

        let page = cache.page(&parent_page_id).unwrap();
        assert_eq!(page.text, "- new\n");
        assert_eq!(page.child_page_ids, vec![child_page_id]);
    }

    #[test]
    fn refresh_page_content_updates_only_touched_ref_targets() {
        let mut cache = WorkspaceCache::new();
        let source_page_id = PageId::new(["A"]).unwrap();
        cache.upsert_page(
            Page::new(source_page_id.clone(), "- [[B]]\n")
                .with_blocks(parse_blocks("- [[B]]\n").unwrap()),
        );
        cache.upsert_page(page(&["B"], "", Vec::new()));
        cache.upsert_page(page(&["C"], "", Vec::new()));
        cache.upsert_page(
            Page::new(PageId::new(["X"]).unwrap(), "- [[C]]\n")
                .with_blocks(parse_blocks("- [[C]]\n").unwrap()),
        );

        cache.refresh_page_content(
            Page::new(source_page_id, "- [[C]]\n").with_blocks(parse_blocks("- [[C]]\n").unwrap()),
        );

        assert_eq!(cache.incoming_refs(&PageId::new(["B"]).unwrap()).len(), 0);
        let c_incoming_refs = cache.incoming_refs(&PageId::new(["C"]).unwrap());
        assert_eq!(c_incoming_refs.len(), 2);
        assert!(
            c_incoming_refs
                .iter()
                .any(|incoming_ref| incoming_ref.source_page_id == PageId::new(["A"]).unwrap())
        );
        assert!(
            c_incoming_refs
                .iter()
                .any(|incoming_ref| incoming_ref.source_page_id == PageId::new(["X"]).unwrap())
        );
    }
}

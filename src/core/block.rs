use super::{FileFingerprint, PageId, SourceSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaintextKind {
    Explicit,
    Implicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Outliner,
    Plaintext(PlaintextKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRefOccurrence {
    pub target_page_id: PageId,
    pub ref_span: SourceSpan,
}

impl PageRefOccurrence {
    pub fn new(target_page_id: PageId, ref_span: SourceSpan) -> Self {
        Self {
            target_page_id,
            ref_span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub block_span: SourceSpan,
    pub content_span: SourceSpan,
    pub children: Vec<Block>,
    pub outgoing_refs: Vec<PageRefOccurrence>,
}

impl Block {
    pub fn new(
        kind: BlockKind,
        block_span: SourceSpan,
        content_span: SourceSpan,
        children: Vec<Block>,
        outgoing_refs: Vec<PageRefOccurrence>,
    ) -> Self {
        debug_assert!(block_span.contains_span(content_span));
        Self {
            kind,
            block_span,
            content_span,
            children,
            outgoing_refs,
        }
    }

    pub fn leaf(kind: BlockKind, block_span: SourceSpan, content_span: SourceSpan) -> Self {
        Self::new(kind, block_span, content_span, Vec::new(), Vec::new())
    }

    pub fn outliner(block_span: SourceSpan, content_span: SourceSpan) -> Self {
        Self::leaf(BlockKind::Outliner, block_span, content_span)
    }

    pub fn explicit_plaintext(block_span: SourceSpan, content_span: SourceSpan) -> Self {
        Self::leaf(
            BlockKind::Plaintext(PlaintextKind::Explicit),
            block_span,
            content_span,
        )
    }

    pub fn implicit_plaintext(block_span: SourceSpan, content_span: SourceSpan) -> Self {
        Self::leaf(
            BlockKind::Plaintext(PlaintextKind::Implicit),
            block_span,
            content_span,
        )
    }

    pub fn walk(&self) -> BlockWalk<'_> {
        BlockWalk { stack: vec![self] }
    }

    pub fn outgoing_refs_recursive(&self) -> impl Iterator<Item = &PageRefOccurrence> {
        self.walk().flat_map(|block| block.outgoing_refs.iter())
    }

    pub fn find_by_span(&self, block_span: SourceSpan) -> Option<&Block> {
        self.walk().find(|block| block.block_span == block_span)
    }
}

pub struct BlockWalk<'a> {
    stack: Vec<&'a Block>,
}

impl<'a> Iterator for BlockWalk<'a> {
    type Item = &'a Block;

    fn next(&mut self) -> Option<Self::Item> {
        let block = self.stack.pop()?;
        self.stack.extend(block.children.iter().rev());
        Some(block)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingRef {
    pub source_page_id: PageId,
    pub source_page_fingerprint: FileFingerprint,
    pub source_block_span: SourceSpan,
    pub ref_span: SourceSpan,
}

impl IncomingRef {
    pub fn new(
        source_page_id: PageId,
        source_page_fingerprint: FileFingerprint,
        source_block_span: SourceSpan,
        ref_span: SourceSpan,
    ) -> Self {
        Self {
            source_page_id,
            source_page_fingerprint,
            source_block_span,
            ref_span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_blocks_in_source_order() {
        let child = Block::outliner(SourceSpan::unchecked(4, 8), SourceSpan::unchecked(6, 8));
        let parent = Block::new(
            BlockKind::Outliner,
            SourceSpan::unchecked(0, 8),
            SourceSpan::unchecked(2, 3),
            vec![child],
            Vec::new(),
        );

        assert_eq!(
            parent
                .walk()
                .map(|block| block.block_span)
                .collect::<Vec<_>>(),
            vec![SourceSpan::unchecked(0, 8), SourceSpan::unchecked(4, 8)]
        );
    }

    #[test]
    fn finds_block_by_span_without_a_block_id() {
        let child = Block::outliner(SourceSpan::unchecked(4, 8), SourceSpan::unchecked(6, 8));
        let parent = Block::new(
            BlockKind::Outliner,
            SourceSpan::unchecked(0, 8),
            SourceSpan::unchecked(2, 3),
            vec![child],
            Vec::new(),
        );

        assert_eq!(
            parent
                .find_by_span(SourceSpan::unchecked(4, 8))
                .unwrap()
                .content_span,
            SourceSpan::unchecked(6, 8)
        );
    }

    #[test]
    fn distinguishes_explicit_and_implicit_plaintext_blocks() {
        let explicit =
            Block::explicit_plaintext(SourceSpan::unchecked(0, 6), SourceSpan::unchecked(3, 6));
        let implicit =
            Block::implicit_plaintext(SourceSpan::unchecked(0, 4), SourceSpan::unchecked(0, 4));

        assert_eq!(explicit.kind, BlockKind::Plaintext(PlaintextKind::Explicit));
        assert_eq!(implicit.kind, BlockKind::Plaintext(PlaintextKind::Implicit));
    }
}

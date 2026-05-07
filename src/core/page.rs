use std::path::PathBuf;

use super::{Block, IncomingRef, PageId, PageLocation, PagePathError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileFingerprint {
    len_bytes: usize,
    content_hash: u64,
}

impl FileFingerprint {
    pub fn from_text(text: &str) -> Self {
        Self {
            len_bytes: text.len(),
            content_hash: fnv1a64(text.as_bytes()),
        }
    }

    pub fn len_bytes(self) -> usize {
        self.len_bytes
    }

    pub fn content_hash(self) -> u64 {
        self.content_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub page_id: PageId,
    pub location: PageLocation,
    pub title: String,
    pub workspace_path: PathBuf,
    pub text: String,
    pub child_page_ids: Vec<PageId>,
    pub blocks: Vec<Block>,
    pub incoming_refs: Vec<IncomingRef>,
    pub fingerprint: FileFingerprint,
}

impl Page {
    pub fn new(page_id: PageId, text: impl Into<String>) -> Self {
        Self::new_in_location(page_id, PageLocation::Pages, text)
            .expect("page-backed paths are always valid")
    }

    pub fn new_in_location(
        page_id: PageId,
        location: PageLocation,
        text: impl Into<String>,
    ) -> Result<Self, PagePathError> {
        if page_id.location() != &location {
            return Err(PagePathError::NestedPath);
        }
        let title = page_id.leaf_name().as_str().to_owned();
        let workspace_path = location.workspace_path_for_page_id(&page_id)?;
        let text = text.into();
        let fingerprint = FileFingerprint::from_text(&text);

        Ok(Self {
            page_id,
            location,
            title,
            workspace_path,
            text,
            child_page_ids: Vec::new(),
            blocks: Vec::new(),
            incoming_refs: Vec::new(),
            fingerprint,
        })
    }

    pub fn with_blocks(mut self, blocks: Vec<Block>) -> Self {
        self.blocks = blocks;
        self
    }

    pub fn set_text_and_blocks(&mut self, text: impl Into<String>, blocks: Vec<Block>) {
        self.text = text.into();
        self.blocks = blocks;
        self.fingerprint = FileFingerprint::from_text(&self.text);
    }

    pub fn find_block_by_span(&self, block_span: super::SourceSpan) -> Option<&Block> {
        self.blocks
            .iter()
            .find_map(|block| block.find_by_span(block_span))
    }

    pub fn outgoing_refs(&self) -> impl Iterator<Item = &super::PageRefOccurrence> {
        self.blocks
            .iter()
            .flat_map(|block| block.outgoing_refs_recursive())
    }

    pub fn parent_page_id(&self) -> Option<PageId> {
        self.location.parent_page_id(&self.page_id)
    }

    pub fn ancestor_page_ids(&self) -> Vec<PageId> {
        self.location.ancestor_page_ids(&self.page_id)
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_when_exact_text_changes() {
        let lf = FileFingerprint::from_text("- A\n");
        let crlf = FileFingerprint::from_text("- A\r\n");

        assert_ne!(lf, crlf);
        assert_eq!(lf.len_bytes(), 4);
        assert_eq!(crlf.len_bytes(), 5);
    }

    #[test]
    fn page_uses_page_id_for_title_and_path() {
        let page_id = PageId::new(["A", "B"]).unwrap();
        let page = Page::new(page_id, "");

        assert_eq!(page.title, "B");
        assert_eq!(page.workspace_path, PathBuf::from("pages").join("A___B.md"));
    }

    #[test]
    fn page_stores_exact_text_and_fingerprint_together() {
        let page_id = PageId::new(["A"]).unwrap();
        let page = Page::new(page_id, "- A\r\n");

        assert_eq!(page.text, "- A\r\n");
        assert_eq!(page.fingerprint, FileFingerprint::from_text("- A\r\n"));
    }

    #[test]
    fn stream_pages_keep_stream_path_but_have_no_parent_page() {
        let page = Page::new_in_location(
            PageId::stream(
                super::super::PageName::new("journal").unwrap(),
                super::super::PageName::new("2026-05-07").unwrap(),
            )
            .unwrap(),
            PageLocation::Stream {
                stream_name: super::super::PageName::new("journal").unwrap(),
            },
            "",
        )
        .unwrap();

        assert_eq!(
            page.workspace_path,
            PathBuf::from("streams").join("journal").join("2026-05-07.md")
        );
        assert!(page.parent_page_id().is_none());
    }
}

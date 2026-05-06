use std::path::PathBuf;

use super::{Block, IncomingRef, PageId};

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
    pub title: String,
    pub workspace_path: PathBuf,
    pub child_page_ids: Vec<PageId>,
    pub blocks: Vec<Block>,
    pub incoming_refs: Vec<IncomingRef>,
    pub fingerprint: FileFingerprint,
}

impl Page {
    pub fn new(page_id: PageId, fingerprint: FileFingerprint) -> Self {
        let title = page_id.leaf_name().as_str().to_owned();
        let workspace_path = page_id.to_workspace_path();

        Self {
            page_id,
            title,
            workspace_path,
            child_page_ids: Vec::new(),
            blocks: Vec::new(),
            incoming_refs: Vec::new(),
            fingerprint,
        }
    }

    pub fn with_blocks(mut self, blocks: Vec<Block>) -> Self {
        self.blocks = blocks;
        self
    }

    pub fn set_blocks(&mut self, blocks: Vec<Block>, fingerprint: FileFingerprint) {
        self.blocks = blocks;
        self.fingerprint = fingerprint;
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
        let page = Page::new(page_id, FileFingerprint::from_text(""));

        assert_eq!(page.title, "B");
        assert_eq!(page.workspace_path, PathBuf::from("A___B.md"));
    }
}

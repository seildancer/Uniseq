mod block;
mod discovery;
mod error;
mod page;
mod page_id;
mod parser;
mod read;
mod span;
mod workspace;

pub use block::{Block, BlockHandle, BlockKind, IncomingRef, PageRefOccurrence, PlaintextKind};
pub use discovery::{WorkspaceDiscovery, discover_workspace, materialize_parent_pages};
pub use error::{CoreError, NameError, PagePathError, ParserError, SpanError};
pub use page::{FileFingerprint, Page};
pub use page_id::{PageId, PageName};
pub use parser::parse_blocks;
pub use read::{
    BlockSnapshot, BlockSnapshotKind, LinkedRefEntry, OutgoingPageRefSnapshot, PageDetail,
    PageSummary, WorkspaceReadApi,
};
pub use span::SourceSpan;
pub use workspace::{IncrementalUpdate, WorkspaceCache};

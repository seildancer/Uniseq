mod block;
mod discovery;
mod error;
mod files;
mod page;
mod page_id;
mod parser;
mod read;
mod session;
mod span;
mod storage;
mod structure;
mod workspace;

pub use block::{Block, BlockKind, IncomingRef, PageRefOccurrence, PlaintextKind};
pub use discovery::{WorkspaceDiscovery, discover_workspace, materialize_parent_pages};
pub use error::{CoreError, NameError, PagePathError, ParserError, SpanError};
pub use page::{FileFingerprint, Page};
pub use page_id::{
    PageId, PageLocation, PageName, ResolvedWorkspacePath, resolve_workspace_path,
    supported_workspace_markdown_path,
};
pub use parser::parse_blocks;
pub use read::{
    BlockSnapshot, BlockSnapshotKind, IncomingPageRefSnapshot, OutgoingPageRefSnapshot,
    PageBlocksSnapshot, PageDetail, PageSummary, WorkspaceReadApi,
};
pub use session::{WatcherFallbackReason, WatcherMode, WorkspaceEvent, WorkspaceSession};
pub use span::SourceSpan;
pub use structure::{
    PageCreate, PageDeleteSubtree, PageMove, PageRename, StreamPageCreate, StreamPageDelete,
    apply_page_create, apply_page_delete_subtree, apply_page_move, apply_page_rename,
    apply_stream_page_create, apply_stream_page_delete,
};
pub use workspace::WorkspaceCache;

mod block;
mod discovery;
mod error;
mod page;
mod page_id;
mod span;
mod workspace;

pub use block::{Block, IncomingRef, PageRefOccurrence};
pub use discovery::{WorkspaceDiscovery, discover_workspace, materialize_parent_pages};
pub use error::{CoreError, NameError, PagePathError, SpanError};
pub use page::{FileFingerprint, Page};
pub use page_id::{PageId, PageName};
pub use span::SourceSpan;
pub use workspace::{IncrementalUpdate, WorkspaceCache};

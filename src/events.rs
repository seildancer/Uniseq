use std::path::PathBuf;

use crate::model::PageKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    WorkspaceOpened { root: PathBuf },
    FileChanged { path: PathBuf },
    IndexRebuilt,
    PageViewInvalidated { page: PageKey },
    SearchIndexUpdated,
    CacheCleared,
}

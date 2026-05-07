use std::fs;
use std::path::Path;

use super::{CoreError, Page, PageId, WorkspaceCache, discover_workspace, parse_blocks};

pub(crate) fn refresh_workspace_cache(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
) -> Result<(), CoreError> {
    *cache = load_workspace_cache(root)?;
    Ok(())
}

pub(crate) fn load_workspace_cache(root: impl AsRef<Path>) -> Result<WorkspaceCache, CoreError> {
    Ok(discover_workspace(root)?.cache)
}

pub(crate) fn load_page_from_relative_path(
    root: &Path,
    relative_path: &Path,
) -> Result<Page, CoreError> {
    let page_id = PageId::from_workspace_path(relative_path)?;
    let absolute_path = root.join(relative_path);
    let text =
        fs::read_to_string(&absolute_path).map_err(|error| CoreError::io(absolute_path, &error))?;
    page_from_markdown(page_id, text)
}

pub(crate) fn page_from_markdown(page_id: PageId, text: String) -> Result<Page, CoreError> {
    let blocks = parse_blocks(&text)?;
    Ok(Page::new(page_id, text).with_blocks(blocks))
}

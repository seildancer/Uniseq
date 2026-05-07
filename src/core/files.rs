use std::fs;
use std::path::Path;

use super::{
    CoreError, Page, WorkspaceCache, discover_workspace, parse_blocks, resolve_workspace_path,
};

pub(crate) fn load_workspace_cache(root: impl AsRef<Path>) -> Result<WorkspaceCache, CoreError> {
    Ok(discover_workspace(root)?.cache)
}

pub(crate) fn load_page_from_relative_path(
    root: &Path,
    relative_path: &Path,
) -> Result<Page, CoreError> {
    let resolved = resolve_workspace_path(relative_path)?;
    let absolute_path = root.join(relative_path);
    let text =
        fs::read_to_string(&absolute_path).map_err(|error| CoreError::io(absolute_path, &error))?;
    page_from_markdown_in_location(resolved.page_id, resolved.location, text)
}

pub(crate) fn page_from_markdown_in_location(
    page_id: super::PageId,
    location: super::PageLocation,
    text: String,
) -> Result<Page, CoreError> {
    let blocks = parse_blocks(&text)?;
    Ok(Page::new_in_location(page_id, location, text)?.with_blocks(blocks))
}

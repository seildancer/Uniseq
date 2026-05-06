use std::fs;
use std::path::Path;

use super::{
    BlockSubtreeEdit, CoreError, FileFingerprint, Page, PageId, SourceSpan, WorkspaceCache,
    parse_blocks,
};

pub fn apply_block_subtree_edit(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    edit: BlockSubtreeEdit,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    let page_id = edit.block_handle.source_page_id();
    let relative_path = page_id.to_workspace_path();
    let absolute_path = root.join(&relative_path);
    let current_text = fs::read_to_string(&absolute_path)
        .map_err(|error| CoreError::io(&absolute_path, &error))?;

    if FileFingerprint::from_text(&current_text) != edit.block_handle.source_page_fingerprint() {
        return Err(CoreError::StalePageRevision);
    }

    let current_page = page_from_markdown(page_id.clone(), current_text)?;
    current_page
        .find_block_by_span(edit.block_handle.block_span())
        .ok_or(CoreError::MissingBlock)?;

    let updated_text = replace_source_region(
        &current_page.text,
        edit.block_handle.block_span(),
        &edit.replacement_markdown,
    )?;

    // Validate the resulting markdown before touching disk.
    page_from_markdown(page_id.clone(), updated_text.clone())?;

    fs::write(&absolute_path, &updated_text)
        .map_err(|error| CoreError::io(&absolute_path, &error))?;
    cache.reparse_and_upsert_page_markdown(page_id, updated_text)?;
    Ok(())
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

fn page_from_markdown(page_id: PageId, text: String) -> Result<Page, CoreError> {
    let blocks = parse_blocks(&text)?;
    Ok(Page::new(page_id, text).with_blocks(blocks))
}

fn replace_source_region(
    text: &str,
    replaced_block_span: SourceSpan,
    replacement_markdown: &str,
) -> Result<String, CoreError> {
    replaced_block_span.validate_for_text(text)?;
    let mut updated_text =
        String::with_capacity(text.len() - replaced_block_span.len() + replacement_markdown.len());
    updated_text.push_str(&text[..replaced_block_span.start()]);
    updated_text.push_str(replacement_markdown);
    updated_text.push_str(&text[replaced_block_span.end()..]);
    Ok(updated_text)
}

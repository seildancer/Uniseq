use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::core::{CoreError, PageId, PageName, PageRefOccurrence, WorkspaceCache, parse_blocks};

use super::OperationKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PageMapping {
    pub(super) old_page_id: PageId,
    pub(super) new_page_id: PageId,
    pub(super) old_path: PathBuf,
    pub(super) new_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileChange {
    pub(super) original_path: PathBuf,
    pub(super) final_path: PathBuf,
    pub(super) original_text: String,
    pub(super) final_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RenameTransactionPlan {
    pub(super) kind: OperationKind,
    pub(super) page_mappings: Vec<PageMapping>,
    pub(super) file_changes: Vec<FileChange>,
    pub(super) deletes: Vec<PathBuf>,
}

pub(super) fn plan_transaction(
    cache: &WorkspaceCache,
    kind: OperationKind,
    source_page_id: &PageId,
    target_page_id: &PageId,
    destination_parent_page_id: Option<&PageId>,
) -> Result<RenameTransactionPlan, CoreError> {
    if cache.page(source_page_id).is_none() {
        return Err(CoreError::MissingPage);
    }

    if kind == OperationKind::Move {
        if let Some(parent_page_id) = destination_parent_page_id {
            if cache.page(parent_page_id).is_none() {
                return Err(CoreError::MissingDestinationParent);
            }
        }
    }

    let page_mappings = cache
        .pages()
        .keys()
        .filter(|page_id| page_id_has_prefix(page_id, source_page_id))
        .map(|old_page_id| {
            let new_page_id = replace_page_id_prefix(old_page_id, source_page_id, target_page_id)?;
            Ok(PageMapping {
                old_page_id: old_page_id.clone(),
                new_page_id: new_page_id.clone(),
                old_path: old_page_id.to_workspace_path(),
                new_path: new_page_id.to_workspace_path(),
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;

    if page_mappings.is_empty() {
        return Err(CoreError::MissingPage);
    }

    validate_destination_paths(cache, &page_mappings)?;

    let moved_page_ids = page_mappings
        .iter()
        .map(|mapping| (mapping.old_page_id.clone(), mapping.new_page_id.clone()))
        .collect::<BTreeMap<_, _>>();

    let file_changes = cache
        .pages()
        .values()
        .filter_map(|page| plan_file_change(page, &moved_page_ids))
        .collect::<Result<Vec<_>, CoreError>>()?;

    let deletes = page_mappings
        .iter()
        .filter(|mapping| mapping.old_path != mapping.new_path)
        .map(|mapping| mapping.old_path.clone())
        .collect::<Vec<_>>();

    Ok(RenameTransactionPlan {
        kind,
        page_mappings,
        file_changes,
        deletes,
    })
}

pub(super) fn renamed_page_id(
    source_page_id: &PageId,
    new_leaf_name: &PageName,
) -> Result<PageId, CoreError> {
    let mut segments = source_page_id.segments().to_vec();
    *segments
        .last_mut()
        .expect("page ids always contain at least one segment") = new_leaf_name.clone();
    Ok(PageId::from_page_names(segments)?)
}

pub(super) fn moved_page_id(
    source_page_id: &PageId,
    destination_parent_page_id: Option<&PageId>,
) -> Result<PageId, CoreError> {
    let mut segments = destination_parent_page_id
        .map(|page_id| page_id.segments().to_vec())
        .unwrap_or_default();
    segments.push(source_page_id.leaf_name().clone());
    Ok(PageId::from_page_names(segments)?)
}

pub(super) fn page_id_has_prefix(page_id: &PageId, prefix: &PageId) -> bool {
    page_id.segments().starts_with(prefix.segments())
}

fn validate_destination_paths(
    cache: &WorkspaceCache,
    page_mappings: &[PageMapping],
) -> Result<(), CoreError> {
    let existing_paths = cache
        .pages()
        .values()
        .map(|page| page.workspace_path.clone())
        .collect::<BTreeSet<_>>();
    let moved_old_paths = page_mappings
        .iter()
        .map(|mapping| mapping.old_path.clone())
        .collect::<BTreeSet<_>>();
    let mut new_paths = BTreeSet::new();

    for mapping in page_mappings {
        if !new_paths.insert(mapping.new_path.clone()) {
            return Err(CoreError::DestinationPageExists);
        }

        if mapping.old_path != mapping.new_path
            && existing_paths.contains(&mapping.new_path)
            && !moved_old_paths.contains(&mapping.new_path)
        {
            return Err(CoreError::DestinationPageExists);
        }
    }

    Ok(())
}

fn plan_file_change(
    page: &crate::core::Page,
    moved_page_ids: &BTreeMap<PageId, PageId>,
) -> Option<Result<FileChange, CoreError>> {
    let final_page_id = moved_page_ids
        .get(&page.page_id)
        .cloned()
        .unwrap_or_else(|| page.page_id.clone());
    let final_path = final_page_id.to_workspace_path();
    let final_text = match rewrite_page_refs(&page.text, page.outgoing_refs(), moved_page_ids) {
        Ok(text) => text,
        Err(error) => return Some(Err(error)),
    };
    let path_changed = final_path != page.workspace_path;
    let content_changed = final_text != page.text;

    if !path_changed && !content_changed {
        return None;
    }

    if let Err(error) = parse_blocks(&final_text) {
        return Some(Err(error));
    }

    Some(Ok(FileChange {
        original_path: page.workspace_path.clone(),
        final_path,
        original_text: page.text.clone(),
        final_text,
    }))
}

fn rewrite_page_refs<'a>(
    text: &str,
    refs: impl Iterator<Item = &'a PageRefOccurrence>,
    moved_page_ids: &BTreeMap<PageId, PageId>,
) -> Result<String, CoreError> {
    let mut rewrites = refs
        .filter_map(|page_ref| {
            moved_page_ids
                .get(&page_ref.target_page_id)
                .map(|new_page_id| (page_ref.ref_span, new_page_id.clone()))
        })
        .collect::<Vec<_>>();

    if rewrites.is_empty() {
        return Ok(text.to_owned());
    }

    rewrites.sort_by_key(|(ref_span, _)| ref_span.start());
    rewrites.reverse();

    let mut updated = text.to_owned();
    for (ref_span, new_page_id) in rewrites {
        let original_ref = ref_span.slice(text)?;
        let replacement = if original_ref.starts_with("[[") {
            format!("[[{}]]", new_page_id.hierarchy_display())
        } else {
            format!("#{}", new_page_id.hierarchy_display())
        };
        replace_span_in_string(&mut updated, ref_span, &replacement)?;
    }

    Ok(updated)
}

fn replace_span_in_string(
    text: &mut String,
    span: crate::core::SourceSpan,
    replacement: &str,
) -> Result<(), CoreError> {
    span.validate_for_text(text)?;
    text.replace_range(span.start()..span.end(), replacement);
    Ok(())
}

fn replace_page_id_prefix(
    page_id: &PageId,
    old_prefix: &PageId,
    new_prefix: &PageId,
) -> Result<PageId, CoreError> {
    if !page_id_has_prefix(page_id, old_prefix) {
        return Err(CoreError::InvalidPageMove);
    }

    let mut segments = new_prefix.segments().to_vec();
    segments.extend_from_slice(&page_id.segments()[old_prefix.segments().len()..]);
    Ok(PageId::from_page_names(segments)?)
}

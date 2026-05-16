mod planning;
mod transaction;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, PageId, PageLocation, PageName, WorkspaceCache, resolve_workspace_path};
use crate::core::discovery::materialize_parent_pages;
use crate::core::files::page_from_markdown_in_location;

use planning::{
    RenameTransactionPlan, moved_page_id, page_id_has_prefix, plan_transaction, renamed_page_id,
};
use transaction::TransactionRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRename {
    pub source_page_id: PageId,
    pub new_leaf_name: PageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMove {
    pub source_page_id: PageId,
    pub destination_parent_page_id: Option<PageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCreate {
    pub page_id: PageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDeleteSubtree {
    pub page_id: PageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMerge {
    pub source_page_id: PageId,
    pub target_page_id: PageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamPageCreate {
    pub stream_name: PageName,
    pub date_name: PageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamPageDelete {
    pub stream_name: PageName,
    pub date_name: PageName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationKind {
    Rename,
    Move,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IncrementalWorkspaceUpdate {
    pub(crate) written_paths: Vec<PathBuf>,
    pub(crate) deleted_paths: Vec<PathBuf>,
    pub(crate) changed_page_ids: Vec<PageId>,
    pub(crate) removed_page_ids: Vec<PageId>,
}

impl IncrementalWorkspaceUpdate {
    fn empty() -> Self {
        Self {
            written_paths: Vec::new(),
            deleted_paths: Vec::new(),
            changed_page_ids: Vec::new(),
            removed_page_ids: Vec::new(),
        }
    }

    fn from_plan(plan: &RenameTransactionPlan) -> Self {
        Self {
            written_paths: plan
                .file_changes
                .iter()
                .map(|change| change.final_path.clone())
                .collect(),
            deleted_paths: plan.deletes.clone(),
            changed_page_ids: plan.changed_page_ids.clone(),
            removed_page_ids: plan.removed_page_ids.clone(),
        }
    }
}

impl OperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Move => "move",
            Self::Delete => "delete",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "rename" => Some(Self::Rename),
            "move" => Some(Self::Move),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

pub fn apply_page_create(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageCreate,
) -> Result<(), CoreError> {
    apply_page_create_with_update(root, cache, request).map(|_| ())
}

pub fn apply_stream_page_create(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: StreamPageCreate,
) -> Result<(), CoreError> {
    apply_stream_page_create_with_update(root, cache, request).map(|_| ())
}

pub fn apply_page_delete_subtree(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageDeleteSubtree,
) -> Result<(), CoreError> {
    apply_page_delete_subtree_with_update(root, cache, request).map(|_| ())
}

pub fn apply_stream_page_delete(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: StreamPageDelete,
) -> Result<(), CoreError> {
    apply_stream_page_delete_with_update(root, cache, request).map(|_| ())
}

pub fn apply_page_merge(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageMerge,
) -> Result<(), CoreError> {
    apply_page_merge_with_update(root, cache, request).map(|_| ())
}

pub fn apply_page_rename(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageRename,
) -> Result<(), CoreError> {
    apply_page_rename_with_update(root, cache, request).map(|_| ())
}

pub(crate) fn apply_page_rename_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageRename,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;
    reject_stream_page_operation(cache, &request.source_page_id, "rename")?;

    let target_page_id = renamed_page_id(&request.source_page_id, &request.new_leaf_name)?;
    if target_page_id == request.source_page_id {
        return Ok(IncrementalWorkspaceUpdate::empty());
    }

    plan_and_commit_transaction(
        root,
        cache,
        OperationKind::Rename,
        &request.source_page_id,
        &target_page_id,
        None,
    )
}

pub(crate) fn apply_page_create_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageCreate,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let relative_path = PageLocation::Pages.workspace_path_for_page_id(&request.page_id)?;
    let absolute_path = root.join(&relative_path);
    if cache.page(&request.page_id).is_some() || absolute_path.exists() {
        return Err(CoreError::DestinationPageExists);
    }

    let ancestor_page_ids = request.page_id.ancestors();
    let referrers = pages_referring_to_any(
        cache,
        ancestor_page_ids
            .iter()
            .chain(std::iter::once(&request.page_id)),
    )?;
    let created_ancestors = materialize_parent_pages(root, cache, ancestor_page_ids)?;
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, &error))?;
    }
    fs::write(&absolute_path, "").map_err(|error| CoreError::io(&absolute_path, &error))?;
    cache.upsert_page(crate::core::Page::new(request.page_id.clone(), ""));

    let mut changed_page_ids = created_ancestors.clone();
    changed_page_ids.push(request.page_id.clone());
    changed_page_ids.extend(referrers);
    changed_page_ids.sort();
    changed_page_ids.dedup();

    let mut written_paths = created_ancestors
        .into_iter()
        .filter_map(|page_id| cache.page(&page_id).map(|page| page.workspace_path.clone()))
        .collect::<Vec<_>>();
    written_paths.push(relative_path);

    Ok(IncrementalWorkspaceUpdate {
        written_paths,
        deleted_paths: Vec::new(),
        changed_page_ids,
        removed_page_ids: Vec::new(),
    })
}

pub(crate) fn apply_stream_page_create_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: StreamPageCreate,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let page_id = stream_page_id(&request.stream_name, &request.date_name)?;
    let relative_path =
        stream_location(&request.stream_name).workspace_path_for_page_id(&page_id)?;
    let absolute_path = root.join(&relative_path);
    if cache.page(&page_id).is_some() || absolute_path.exists() {
        return Err(CoreError::DestinationPageExists);
    }

    let referrers = pages_referring_to_any(cache, std::iter::once(&page_id))?;
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, &error))?;
    }
    fs::write(&absolute_path, "").map_err(|error| CoreError::io(&absolute_path, &error))?;
    cache.upsert_page(crate::core::Page::new_in_location(
        page_id.clone(),
        stream_location(&request.stream_name),
        "",
    )?);

    let mut changed_page_ids = vec![page_id];
    changed_page_ids.extend(referrers);
    changed_page_ids.sort();
    changed_page_ids.dedup();

    Ok(IncrementalWorkspaceUpdate {
        written_paths: vec![relative_path],
        deleted_paths: Vec::new(),
        changed_page_ids,
        removed_page_ids: Vec::new(),
    })
}

pub(crate) fn apply_page_delete_subtree_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageDeleteSubtree,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let Some(page) = cache.page(&request.page_id) else {
        return Err(CoreError::MissingPage);
    };
    if page.location.is_stream_backed() {
        return Err(CoreError::UnsupportedStreamOperation {
            operation: "delete_subtree",
        });
    }

    let plan = plan_delete_subtree_transaction(cache, &request.page_id)?;
    let update = IncrementalWorkspaceUpdate::from_plan(&plan);
    let record = TransactionRecord::stage(root, &plan)?;
    complete_transaction_record(root, cache, record, Some(&plan), Some(update))
}

pub(crate) fn apply_stream_page_delete_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: StreamPageDelete,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let page_id = stream_page_id(&request.stream_name, &request.date_name)?;
    let Some(page) = cache.page(&page_id).cloned() else {
        return Err(CoreError::MissingPage);
    };
    if page.location.is_page_backed() {
        return Err(CoreError::MissingPage);
    }

    let changed_page_ids = pages_referring_to_any(cache, std::iter::once(&page_id))?;
    let absolute_path = root.join(&page.workspace_path);
    match fs::remove_file(&absolute_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CoreError::MissingPage);
        }
        Err(error) => return Err(CoreError::io(&absolute_path, &error)),
    }
    cache.remove_page(&page_id);

    Ok(IncrementalWorkspaceUpdate {
        written_paths: Vec::new(),
        deleted_paths: vec![page.workspace_path.clone()],
        changed_page_ids,
        removed_page_ids: vec![page_id],
    })
}

pub(crate) fn apply_page_merge_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageMerge,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;
    validate_page_merge_request(cache, &request)?;

    let source_page = cache
        .page(&request.source_page_id)
        .ok_or(CoreError::MissingPage)?
        .clone();
    let target_page = cache
        .page(&request.target_page_id)
        .ok_or(CoreError::MissingPage)?
        .clone();

    let rewrite_map = BTreeMap::from([(
        request.source_page_id.clone(),
        request.target_page_id.clone(),
    )]);
    let rewritten_source_text =
        planning::rewrite_page_refs(&source_page.text, source_page.outgoing_refs(), &rewrite_map)?;
    let rewritten_target_text =
        planning::rewrite_page_refs(&target_page.text, target_page.outgoing_refs(), &rewrite_map)?;
    let merged_text = merge_page_texts(&rewritten_target_text, &rewritten_source_text);
    let updated_target = page_from_markdown_in_location(
        request.target_page_id.clone(),
        target_page.location.clone(),
        merged_text,
    )?;

    let mut rewritten_pages = Vec::new();
    for referrer_id in pages_referring_to_any(cache, std::iter::once(&request.source_page_id))? {
        if referrer_id == request.source_page_id || referrer_id == request.target_page_id {
            continue;
        }

        let referrer = cache
            .page(&referrer_id)
            .ok_or(CoreError::MissingPage)?
            .clone();
        let rewritten =
            planning::rewrite_page_refs(&referrer.text, referrer.outgoing_refs(), &rewrite_map)?;
        if rewritten == referrer.text {
            continue;
        }

        let updated = page_from_markdown_in_location(
            referrer.page_id.clone(),
            referrer.location.clone(),
            rewritten,
        )?;
        rewritten_pages.push((referrer, updated));
    }

    let mut expected_source_files = vec![
        expected_source_file(&source_page),
        expected_source_file(&target_page),
    ];
    expected_source_files.extend(
        rewritten_pages
            .iter()
            .map(|(page, _)| expected_source_file(page)),
    );
    validate_expected_source_files(root, &expected_source_files)?;

    let target_abs = root.join(&target_page.workspace_path);
    fs::write(&target_abs, &updated_target.text)
        .map_err(|error| CoreError::io(&target_abs, &error))?;

    let mut written_paths = vec![target_page.workspace_path.clone()];
    for (_, updated) in &rewritten_pages {
        let page_abs = root.join(&updated.workspace_path);
        fs::write(&page_abs, &updated.text).map_err(|error| CoreError::io(&page_abs, &error))?;
        written_paths.push(updated.workspace_path.clone());
    }

    let source_abs = root.join(&source_page.workspace_path);
    match fs::remove_file(&source_abs) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CoreError::StructuralConflict {
                path: source_page.workspace_path.clone(),
            });
        }
        Err(error) => return Err(CoreError::io(&source_abs, &error)),
    }

    let mut changed_page_ids =
        changed_page_ids_for_updated_page(cache, &target_page, &updated_target)?;
    for (page, updated) in &rewritten_pages {
        changed_page_ids.extend(changed_page_ids_for_updated_page(cache, page, updated)?);
    }
    if let Some(parent_page_id) = source_page.parent_page_id() {
        if parent_page_id != request.source_page_id
            && parent_page_id != request.target_page_id
            && cache.page(&parent_page_id).is_some()
        {
            changed_page_ids.insert(parent_page_id);
        }
    }
    changed_page_ids.remove(&request.source_page_id);

    cache.refresh_page_content(updated_target);
    for (_, updated) in rewritten_pages {
        cache.refresh_page_content(updated);
    }
    cache.remove_page(&request.source_page_id);

    Ok(IncrementalWorkspaceUpdate {
        written_paths,
        deleted_paths: vec![source_page.workspace_path.clone()],
        changed_page_ids: changed_page_ids.into_iter().collect(),
        removed_page_ids: vec![request.source_page_id],
    })
}

pub fn apply_page_move(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageMove,
) -> Result<(), CoreError> {
    apply_page_move_with_update(root, cache, request).map(|_| ())
}

pub(crate) fn apply_page_move_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageMove,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;
    reject_stream_page_operation(cache, &request.source_page_id, "move")?;

    if request
        .destination_parent_page_id
        .as_ref()
        .is_some_and(|parent| page_id_has_prefix(parent, &request.source_page_id))
    {
        return Err(CoreError::InvalidPageMove);
    }

    let target_page_id = moved_page_id(
        &request.source_page_id,
        request.destination_parent_page_id.as_ref(),
    )?;
    if target_page_id == request.source_page_id {
        return Ok(IncrementalWorkspaceUpdate::empty());
    }

    plan_and_commit_transaction(
        root,
        cache,
        OperationKind::Move,
        &request.source_page_id,
        &target_page_id,
        request.destination_parent_page_id.as_ref(),
    )
}

pub fn recover_workspace_transactions(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
) -> Result<bool, CoreError> {
    let root = root.as_ref();
    if !TransactionRecord::exists(root) {
        return Ok(false);
    }

    let record = TransactionRecord::load(root)?;
    complete_transaction_record(root, cache, record, None, None)?;
    Ok(true)
}

fn plan_and_commit_transaction(
    root: &Path,
    cache: &mut WorkspaceCache,
    operation_kind: OperationKind,
    source_page_id: &PageId,
    target_page_id: &PageId,
    destination_parent_page_id: Option<&PageId>,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let plan = plan_transaction(
        cache,
        operation_kind,
        source_page_id,
        target_page_id,
        destination_parent_page_id,
    )?;
    let record = TransactionRecord::stage(root, &plan)?;
    complete_transaction_record(root, cache, record, Some(&plan), None)
}

fn complete_transaction_record(
    root: &Path,
    cache: &mut WorkspaceCache,
    mut record: TransactionRecord,
    prepared_plan: Option<&RenameTransactionPlan>,
    prepared_update: Option<IncrementalWorkspaceUpdate>,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    // Recovery intentionally finishes the structural transaction to its planned
    // final state rather than attempting rollback. Markdown files remain
    // authoritative, and startup/runtime recovery replays the recorded final
    // state until disk and cache converge on one deterministic committed
    // outcome.
    record.validate_final_paths_available(root)?;
    record.mark_applying(root)?;
    record.apply_final_state(root, None, false)?;
    let update = if let Some(plan) = prepared_plan {
        let update = prepared_update.unwrap_or_else(|| IncrementalWorkspaceUpdate::from_plan(plan));
        apply_transaction_plan_to_cache(cache, plan)?;
        update
    } else {
        let final_writes = record.final_writes(root)?;
        apply_transaction_writes_to_cache(cache, &final_writes, record.deletes())?;
        IncrementalWorkspaceUpdate {
            written_paths: final_writes.iter().map(|(path, _)| path.clone()).collect(),
            deleted_paths: record.deletes().to_vec(),
            changed_page_ids: final_writes
                .iter()
                .map(|(path, _)| PageId::from_workspace_path(path))
                .collect::<Result<Vec<_>, _>>()?,
            removed_page_ids: record
                .deletes()
                .iter()
                .map(PageId::from_workspace_path)
                .collect::<Result<Vec<_>, _>>()?,
        }
    };
    record.remove(root)?;
    Ok(update)
}

fn plan_delete_subtree_transaction(
    cache: &WorkspaceCache,
    root_page_id: &PageId,
) -> Result<RenameTransactionPlan, CoreError> {
    let deleted_pages = cache
        .pages()
        .values()
        .filter(|page| {
            page.location.is_page_backed() && page_id_has_prefix(&page.page_id, root_page_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if deleted_pages.is_empty() {
        return Err(CoreError::MissingPage);
    }

    let removed_page_ids = deleted_pages
        .iter()
        .map(|page| page.page_id.clone())
        .collect::<Vec<_>>();
    let removed_page_id_set = removed_page_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut changed_page_ids = pages_referring_to_any(cache, removed_page_ids.iter())?
        .into_iter()
        .filter(|page_id| !removed_page_id_set.contains(page_id))
        .collect::<Vec<_>>();
    if let Some(parent_page_id) = root_page_id.parent() {
        if !removed_page_id_set.contains(&parent_page_id) && cache.page(&parent_page_id).is_some() {
            changed_page_ids.push(parent_page_id);
        }
    }
    changed_page_ids.sort();
    changed_page_ids.dedup();

    Ok(RenameTransactionPlan {
        kind: OperationKind::Delete,
        page_mappings: Vec::new(),
        file_changes: Vec::new(),
        deletes: deleted_pages
            .iter()
            .map(|page| page.workspace_path.clone())
            .collect(),
        expected_source_files: deleted_pages
            .iter()
            .map(|page| planning::ExpectedSourceFile {
                workspace_path: page.workspace_path.clone(),
                fingerprint: page.fingerprint,
            })
            .collect(),
        changed_page_ids,
        removed_page_ids,
    })
}

#[cfg(test)]
pub(crate) fn stage_page_rename_transaction_for_testing(
    root: impl AsRef<Path>,
    source_page_id: &PageId,
    new_leaf_name: &PageName,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    let target_page_id = renamed_page_id(source_page_id, new_leaf_name)?;
    let disk_cache = crate::core::files::load_workspace_cache(root)?;
    let plan = plan_transaction(
        &disk_cache,
        OperationKind::Rename,
        source_page_id,
        &target_page_id,
        None,
    )?;
    TransactionRecord::stage(root, &plan)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn stage_page_delete_transaction_for_testing(
    root: impl AsRef<Path>,
    page_id: &PageId,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    let disk_cache = crate::core::files::load_workspace_cache(root)?;
    let plan = plan_delete_subtree_transaction(&disk_cache, page_id)?;
    TransactionRecord::stage(root, &plan)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn apply_staged_transaction_partially_for_testing(
    root: impl AsRef<Path>,
    write_limit: Option<usize>,
    skip_deletes: bool,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    let mut record = TransactionRecord::load(root)?;
    record.mark_applying(root)?;
    record.apply_final_state(root, write_limit, skip_deletes)
}

fn apply_transaction_plan_to_cache(
    cache: &mut WorkspaceCache,
    plan: &RenameTransactionPlan,
) -> Result<(), CoreError> {
    let final_writes = plan
        .file_changes
        .iter()
        .map(|change| (change.final_path.clone(), change.final_text.clone()))
        .collect::<Vec<_>>();
    apply_transaction_writes_to_cache(cache, &final_writes, &plan.deletes)
}

fn apply_transaction_writes_to_cache(
    cache: &mut WorkspaceCache,
    final_writes: &[(PathBuf, String)],
    deleted_paths: &[PathBuf],
) -> Result<(), CoreError> {
    let final_pages = final_writes
        .iter()
        .map(|(path, text)| {
            let resolved = resolve_workspace_path(path)?;
            page_from_markdown_in_location(resolved.page_id, resolved.location, text.clone())
        })
        .collect::<Result<Vec<_>, CoreError>>()?;

    for deleted_path in deleted_paths {
        let deleted_page_id = PageId::from_workspace_path(deleted_path)?;
        cache.remove_page(&deleted_page_id);
    }

    for page in final_pages {
        if cache.page(&page.page_id).is_some() {
            cache.refresh_page_content(page);
        } else {
            cache.upsert_page(page);
        }
    }

    Ok(())
}

fn stream_location(stream_name: &PageName) -> PageLocation {
    PageLocation::Stream {
        stream_name: stream_name.clone(),
    }
}

fn pages_referring_to_any<'a>(
    cache: &WorkspaceCache,
    target_page_ids: impl IntoIterator<Item = &'a PageId>,
) -> Result<Vec<PageId>, CoreError> {
    let target_page_ids = target_page_ids
        .into_iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut source_page_ids = BTreeSet::new();

    for target_page_id in target_page_ids {
        for incoming_ref in cache.incoming_refs(&target_page_id) {
            source_page_ids.insert(incoming_ref.source_page_id.clone());
        }
    }

    Ok(source_page_ids.into_iter().collect())
}

fn validate_page_merge_request(
    cache: &WorkspaceCache,
    request: &PageMerge,
) -> Result<(), CoreError> {
    if request.source_page_id == request.target_page_id {
        return Err(CoreError::InvalidPageMerge);
    }

    reject_stream_page_operation(cache, &request.source_page_id, "merge")?;
    reject_stream_page_operation(cache, &request.target_page_id, "merge")?;

    let source_page = cache
        .page(&request.source_page_id)
        .ok_or(CoreError::MissingPage)?;
    cache
        .page(&request.target_page_id)
        .ok_or(CoreError::MissingPage)?;

    if !source_page.child_page_ids.is_empty() {
        return Err(CoreError::InvalidPageMerge);
    }

    Ok(())
}

fn merge_page_texts(target_text: &str, source_text: &str) -> String {
    if source_text.is_empty() {
        return target_text.to_owned();
    }
    if target_text.is_empty() {
        return source_text.to_owned();
    }
    if target_text.ends_with('\n') {
        format!("{target_text}\n{source_text}")
    } else {
        format!("{target_text}\n\n{source_text}")
    }
}

fn expected_source_file(page: &crate::core::Page) -> planning::ExpectedSourceFile {
    planning::ExpectedSourceFile {
        workspace_path: page.workspace_path.clone(),
        fingerprint: page.fingerprint,
    }
}

fn validate_expected_source_files(
    root: &Path,
    expected_source_files: &[planning::ExpectedSourceFile],
) -> Result<(), CoreError> {
    for expected in expected_source_files {
        let absolute_path = root.join(&expected.workspace_path);
        let disk_text = match fs::read_to_string(&absolute_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CoreError::StructuralConflict {
                    path: expected.workspace_path.clone(),
                });
            }
            Err(error) => return Err(CoreError::io(&absolute_path, &error)),
        };

        if crate::core::FileFingerprint::from_text(&disk_text) != expected.fingerprint {
            return Err(CoreError::StructuralConflict {
                path: expected.workspace_path.clone(),
            });
        }
    }

    Ok(())
}

fn changed_page_ids_for_updated_page(
    cache: &WorkspaceCache,
    old_page: &crate::core::Page,
    new_page: &crate::core::Page,
) -> Result<BTreeSet<PageId>, CoreError> {
    let mut changed_page_ids = BTreeSet::from([new_page.page_id.clone()]);
    let mut affected_target_page_ids = target_page_ids_from_page(old_page);
    affected_target_page_ids.extend(target_page_ids_from_page(new_page));

    changed_page_ids.extend(
        affected_target_page_ids
            .iter()
            .filter(|page_id| **page_id == new_page.page_id || cache.page(*page_id).is_some())
            .cloned(),
    );
    changed_page_ids
        .extend(pages_referring_to_any(cache, affected_target_page_ids.iter())?.into_iter());

    Ok(changed_page_ids)
}

fn target_page_ids_from_page(page: &crate::core::Page) -> BTreeSet<PageId> {
    page.outgoing_refs()
        .map(|outgoing_ref| outgoing_ref.target_page_id.clone())
        .collect()
}

fn stream_page_id(stream_name: &PageName, date_name: &PageName) -> Result<PageId, CoreError> {
    Ok(PageId::stream(stream_name.clone(), date_name.clone())?)
}

fn reject_stream_page_operation(
    cache: &WorkspaceCache,
    page_id: &PageId,
    operation: &'static str,
) -> Result<(), CoreError> {
    if cache
        .page(page_id)
        .is_some_and(|page| page.location.is_stream_backed())
    {
        return Err(CoreError::UnsupportedStreamOperation { operation });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CoreError, Page, WorkspaceReadApi,
        core::files::{TestWorkspace, workspace_test_relative_path},
        discover_workspace,
    };

    #[test]
    fn rename_moves_subtree_and_rewrites_inbound_refs() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- [[A/B/C]]\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("X.md", "- [[A/B]] and #A/B/C\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("Renamed").unwrap(),
            },
        )
        .unwrap();

        assert!(!workspace.file_exists("A___B.md"));
        assert!(!workspace.file_exists("A___B___C.md"));
        assert_eq!(workspace.read_file("A___Renamed.md"), "- [[A/Renamed/C]]\n");
        assert_eq!(workspace.read_file("A___Renamed___C.md"), "- child\n");
        assert_eq!(
            workspace.read_file("X.md"),
            "- [[A/Renamed]] and #A/Renamed/C\n"
        );
        assert!(cache.page(&PageId::new(["A", "B"]).unwrap()).is_none());
        assert!(cache.page(&PageId::new(["A", "B", "C"]).unwrap()).is_none());
        assert!(
            cache
                .page(&PageId::new(["A", "Renamed"]).unwrap())
                .is_some()
        );
        assert!(
            cache
                .page(&PageId::new(["A", "Renamed", "C"]).unwrap())
                .is_some()
        );
        assert_eq!(
            cache
                .incoming_refs(&PageId::new(["A", "Renamed"]).unwrap())
                .len(),
            1
        );
        assert_eq!(
            cache
                .incoming_refs(&PageId::new(["A", "Renamed", "C"]).unwrap())
                .len(),
            2
        );
    }

    #[test]
    fn move_relocates_subtree_under_existing_parent_and_rewrites_refs() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- [[A/B/C]]\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("Z.md", "");
        workspace.write_file("X.md", "- [[A/B]] and #A/B/C\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            },
        )
        .unwrap();

        assert!(!workspace.file_exists("A___B.md"));
        assert!(!workspace.file_exists("A___B___C.md"));
        assert_eq!(workspace.read_file("Z___B.md"), "- [[Z/B/C]]\n");
        assert_eq!(workspace.read_file("Z___B___C.md"), "- child\n");
        assert_eq!(workspace.read_file("X.md"), "- [[Z/B]] and #Z/B/C\n");
        assert!(cache.page(&PageId::new(["Z", "B"]).unwrap()).is_some());
        assert!(cache.page(&PageId::new(["Z", "B", "C"]).unwrap()).is_some());
    }

    #[test]
    fn move_rejects_missing_destination_parent() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Missing"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::MissingDestinationParent);
        assert!(workspace.file_exists("A___B.md"));
    }

    #[test]
    fn move_rejects_destination_collisions() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "");
        workspace.write_file("Z.md", "");
        workspace.write_file("Z___B.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::DestinationPageExists);
        assert!(workspace.file_exists("A___B.md"));
        assert!(workspace.file_exists("Z___B.md"));
    }

    #[test]
    fn move_rejects_descendant_destinations() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["A", "B"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::InvalidPageMove);
    }

    #[test]
    fn create_materializes_parent_pages_and_refreshes_cache() {
        let workspace = TestWorkspace::new("uniseq-structure");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_create(
            &workspace.root,
            &mut cache,
            PageCreate {
                page_id: PageId::new(["A", "B", "C"]).unwrap(),
            },
        )
        .unwrap();

        assert!(workspace.file_exists("A.md"));
        assert!(workspace.file_exists("A___B.md"));
        assert!(workspace.file_exists("A___B___C.md"));
        assert!(cache.page(&PageId::new(["A"]).unwrap()).is_some());
        assert!(cache.page(&PageId::new(["A", "B"]).unwrap()).is_some());
        assert!(cache.page(&PageId::new(["A", "B", "C"]).unwrap()).is_some());
    }

    #[test]
    fn create_rejects_existing_pages() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_create(
            &workspace.root,
            &mut cache,
            PageCreate {
                page_id: PageId::new(["A"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::DestinationPageExists);
        assert!(workspace.file_exists("A.md"));
    }

    #[test]
    fn stream_create_and_delete_manage_single_stream_files() {
        let workspace = TestWorkspace::new("uniseq-structure");
        let mut cache = discover_workspace(&workspace.root).unwrap().cache;

        apply_stream_page_create(
            &workspace.root,
            &mut cache,
            StreamPageCreate {
                stream_name: PageName::new("journal").unwrap(),
                date_name: PageName::new("2026_05_07").unwrap(),
            },
        )
        .unwrap();

        assert!(workspace.file_exists("journal/2026_05_07.md"));
        assert!(
            cache
                .page(
                    &PageId::stream(
                        PageName::new("journal").unwrap(),
                        PageName::new("2026_05_07").unwrap(),
                    )
                    .unwrap(),
                )
                .is_some()
        );

        apply_stream_page_delete(
            &workspace.root,
            &mut cache,
            StreamPageDelete {
                stream_name: PageName::new("journal").unwrap(),
                date_name: PageName::new("2026_05_07").unwrap(),
            },
        )
        .unwrap();

        assert!(!workspace.file_exists("journal/2026_05_07.md"));
        assert!(
            cache
                .page(
                    &PageId::stream(
                        PageName::new("journal").unwrap(),
                        PageName::new("2026_05_07").unwrap(),
                    )
                    .unwrap(),
                )
                .is_none()
        );
    }

    #[test]
    fn rename_and_move_reject_stream_pages() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("journal/2026_05_07.md", "- body\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;

        let rename_error = apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026_05_07").unwrap(),
                )
                .unwrap(),
                new_leaf_name: PageName::new("2026_05_08").unwrap(),
            },
        )
        .unwrap_err();
        assert_eq!(
            rename_error,
            CoreError::UnsupportedStreamOperation {
                operation: "rename"
            }
        );

        let move_error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026_05_07").unwrap(),
                )
                .unwrap(),
                destination_parent_page_id: Some(PageId::new(["A"]).unwrap()),
            },
        )
        .unwrap_err();
        assert_eq!(
            move_error,
            CoreError::UnsupportedStreamOperation { operation: "move" }
        );

        let merge_error = apply_page_merge(
            &workspace.root,
            &mut cache,
            PageMerge {
                source_page_id: PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026_05_07").unwrap(),
                )
                .unwrap(),
                target_page_id: PageId::new(["A"]).unwrap(),
            },
        )
        .unwrap_err();
        assert_eq!(
            merge_error,
            CoreError::UnsupportedStreamOperation { operation: "merge" }
        );
    }

    #[test]
    fn delete_removes_page_subtree_and_refreshes_refs() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- child\n");
        workspace.write_file("A___B___C.md", "- grandchild\n");
        workspace.write_file("X.md", "- [[A/B]] and [[A/B/C]]\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_delete_subtree(
            &workspace.root,
            &mut cache,
            PageDeleteSubtree {
                page_id: PageId::new(["A", "B"]).unwrap(),
            },
        )
        .unwrap();

        assert!(workspace.file_exists("A.md"));
        assert!(!workspace.file_exists("A___B.md"));
        assert!(!workspace.file_exists("A___B___C.md"));
        assert!(cache.page(&PageId::new(["A", "B"]).unwrap()).is_none());
        assert!(cache.page(&PageId::new(["A", "B", "C"]).unwrap()).is_none());
        assert_eq!(workspace.read_file("X.md"), "- [[A/B]] and [[A/B/C]]\n");

        let read_api = WorkspaceReadApi::new(&cache, &|_| None);
        let _x_blocks = read_api.page_content(&PageId::new(["X"]).unwrap()).unwrap();
        let x_outgoing = read_api
            .page_outgoing_refs(&PageId::new(["X"]).unwrap())
            .unwrap();
        assert_eq!(x_outgoing.len(), 2);
        assert!(!x_outgoing[0].target_exists);
        assert!(!x_outgoing[1].target_exists);
    }

    #[test]
    fn delete_rejects_missing_pages() {
        let workspace = TestWorkspace::new("uniseq-structure");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_delete_subtree(
            &workspace.root,
            &mut cache,
            PageDeleteSubtree {
                page_id: PageId::new(["Missing"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::MissingPage);
    }

    #[test]
    fn delete_rejects_stale_source_page_content_before_staging() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- cached\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("A___B.md", "- newer\n");

        let error = apply_page_delete_subtree(
            &workspace.root,
            &mut cache,
            PageDeleteSubtree {
                page_id: PageId::new(["A", "B"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("A___B.md")
            }
        );
        assert!(workspace.file_exists("A___B.md"));
    }

    #[test]
    fn merge_appends_content_and_rewrites_refs() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "- source [[A]] and [[C]]\n");
        workspace.write_file("B.md", "- target [[A]]\n");
        workspace.write_file("C.md", "");
        workspace.write_file("X.md", "- [[A]] and #A\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_merge(
            &workspace.root,
            &mut cache,
            PageMerge {
                source_page_id: PageId::new(["A"]).unwrap(),
                target_page_id: PageId::new(["B"]).unwrap(),
            },
        )
        .unwrap();

        assert!(!workspace.file_exists("A.md"));
        assert_eq!(
            workspace.read_file("B.md"),
            "- target [[B]]\n\n- source [[B]] and [[C]]\n"
        );
        assert_eq!(workspace.read_file("X.md"), "- [[B]] and #B\n");
        assert!(cache.page(&PageId::new(["A"]).unwrap()).is_none());

        let read_api = WorkspaceReadApi::new(&cache, &|_| None);
        let outgoing_refs = read_api
            .page_outgoing_refs(&PageId::new(["B"]).unwrap())
            .unwrap();
        assert_eq!(outgoing_refs.len(), 3);
        assert_eq!(outgoing_refs[0].target_page_id, PageId::new(["B"]).unwrap());
        assert!(outgoing_refs[0].target_exists);
        assert_eq!(outgoing_refs[1].target_page_id, PageId::new(["B"]).unwrap());
        assert!(outgoing_refs[1].target_exists);
        assert_eq!(outgoing_refs[2].target_page_id, PageId::new(["C"]).unwrap());
        assert!(outgoing_refs[2].target_exists);

        let incoming_refs = read_api
            .page_incoming_refs(&PageId::new(["B"]).unwrap())
            .unwrap();
        assert_eq!(incoming_refs.len(), 4);
    }

    #[test]
    fn merge_rejects_same_source_and_target() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "- body\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_merge(
            &workspace.root,
            &mut cache,
            PageMerge {
                source_page_id: PageId::new(["A"]).unwrap(),
                target_page_id: PageId::new(["A"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::InvalidPageMerge);
    }

    #[test]
    fn merge_rejects_source_pages_with_children() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "- body\n");
        workspace.write_file("A___Child.md", "- child\n");
        workspace.write_file("B.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = apply_page_merge(
            &workspace.root,
            &mut cache,
            PageMerge {
                source_page_id: PageId::new(["A"]).unwrap(),
                target_page_id: PageId::new(["B"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::InvalidPageMerge);
    }

    #[test]
    fn merge_rejects_stale_referrer_source_before_write() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "- body\n");
        workspace.write_file("B.md", "- target\n");
        workspace.write_file("X.md", "- [[A]]\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("X.md", "- [[A]] and updated\n");

        let error = apply_page_merge(
            &workspace.root,
            &mut cache,
            PageMerge {
                source_page_id: PageId::new(["A"]).unwrap(),
                target_page_id: PageId::new(["B"]).unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("X.md"),
            }
        );
        assert_eq!(workspace.read_file("A.md"), "- body\n");
        assert_eq!(workspace.read_file("B.md"), "- target\n");
        assert_eq!(workspace.read_file("X.md"), "- [[A]] and updated\n");
    }

    #[test]
    fn rename_updates_normalized_incoming_refs_after_commit() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("X.md", "- [[A/B]]\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("C").unwrap(),
            },
        )
        .unwrap();

        let read_api = WorkspaceReadApi::new(&cache, &|_| None);
        let incoming_refs = read_api
            .page_incoming_refs(&PageId::new(["A", "C"]).unwrap())
            .unwrap();
        assert_eq!(incoming_refs.len(), 1);
        assert_eq!(incoming_refs[0].source_page_id, PageId::new(["X"]).unwrap());
    }

    #[test]
    fn recovery_finishes_interrupted_transaction_and_refreshes_cache() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("X.md", "- [[A/B]]\n");

        let disk_cache = crate::core::files::load_workspace_cache(&workspace.root).unwrap();
        let plan = plan_transaction(
            &disk_cache,
            OperationKind::Rename,
            &PageId::new(["A", "B"]).unwrap(),
            &PageId::new(["A", "Renamed"]).unwrap(),
            None,
        )
        .unwrap();
        TransactionRecord::stage(&workspace.root, &plan).unwrap();
        let mut record = TransactionRecord::load(&workspace.root).unwrap();
        record.mark_applying(&workspace.root).unwrap();
        record
            .apply_final_state(&workspace.root, Some(1), true)
            .unwrap();

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        assert!(recover_workspace_transactions(&workspace.root, &mut cache).unwrap());

        assert_eq!(workspace.read_file("A___Renamed.md"), "- body\n");
        assert_eq!(workspace.read_file("A___Renamed___C.md"), "- child\n");
        assert_eq!(workspace.read_file("X.md"), "- [[A/Renamed]]\n");
        assert!(!workspace.file_exists("A___B.md"));
        assert!(!workspace.file_exists("A___B___C.md"));
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
        assert!(
            cache
                .page(&PageId::new(["A", "Renamed"]).unwrap())
                .is_some()
        );
        assert!(
            cache
                .page(&PageId::new(["A", "Renamed", "C"]).unwrap())
                .is_some()
        );
        drop(record);
    }

    #[test]
    fn delete_recovery_finishes_interrupted_transaction_and_refreshes_cache() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("X.md", "- [[A/B]]\n");

        stage_page_delete_transaction_for_testing(
            &workspace.root,
            &PageId::new(["A", "B"]).unwrap(),
        )
        .unwrap();
        apply_staged_transaction_partially_for_testing(&workspace.root, Some(0), true).unwrap();

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        assert!(recover_workspace_transactions(&workspace.root, &mut cache).unwrap());

        assert!(workspace.file_exists("A.md"));
        assert!(!workspace.file_exists("A___B.md"));
        assert!(!workspace.file_exists("A___B___C.md"));
        assert_eq!(workspace.read_file("X.md"), "- [[A/B]]\n");
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
        assert!(cache.page(&PageId::new(["A", "B"]).unwrap()).is_none());
        assert!(cache.page(&PageId::new(["A", "B", "C"]).unwrap()).is_none());
    }

    #[test]
    fn external_collision_after_read_rejects_without_mutating_workspace() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "");
        workspace.write_file("Z.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let _page = Page::new(PageId::new(["A", "B"]).unwrap(), "");
        workspace.write_file("Z___B.md", "- external\n");

        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::DestinationPageExists);
        assert!(workspace.file_exists("A___B.md"));
        assert_eq!(workspace.read_file("Z___B.md"), "- external\n");
    }

    #[test]
    fn rename_rejects_stale_source_page_content_before_staging() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- cached\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("A___B.md", "- newer\n");

        let error = apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("C").unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("A___B.md"),
            }
        );
        assert_eq!(workspace.read_file("A___B.md"), "- newer\n");
        assert!(!workspace.file_exists("A___C.md"));
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn rename_rejects_stale_inbound_ref_source_before_staging() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("X.md", "- [[A/B]]\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("X.md", "- [[A/B]] and updated\n");

        let error = apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("C").unwrap(),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("X.md"),
            }
        );
        assert_eq!(workspace.read_file("A___B.md"), "- body\n");
        assert_eq!(workspace.read_file("X.md"), "- [[A/B]] and updated\n");
        assert!(!workspace.file_exists("A___C.md"));
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn move_rejects_stale_source_page_content_before_staging() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- cached\n");
        workspace.write_file("Z.md", "");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("A___B.md", "- newer\n");

        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("A___B.md"),
            }
        );
        assert_eq!(workspace.read_file("A___B.md"), "- newer\n");
        assert!(!workspace.file_exists("Z___B.md"));
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn move_rejects_stale_inbound_ref_source_before_staging() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("Z.md", "");
        workspace.write_file("X.md", "- [[A/B]]\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        workspace.write_file("X.md", "- [[A/B]] and updated\n");

        let error = apply_page_move(
            &workspace.root,
            &mut cache,
            PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            CoreError::StructuralConflict {
                path: workspace_test_relative_path("X.md"),
            }
        );
        assert_eq!(workspace.read_file("A___B.md"), "- body\n");
        assert_eq!(workspace.read_file("X.md"), "- [[A/B]] and updated\n");
        assert!(!workspace.file_exists("Z___B.md"));
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn recovery_rejects_late_destination_collisions_before_commit() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");

        stage_page_rename_transaction_for_testing(
            &workspace.root,
            &PageId::new(["A", "B"]).unwrap(),
            &PageName::new("C").unwrap(),
        )
        .unwrap();
        workspace.write_file("A___C.md", "- external\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;
        let error = recover_workspace_transactions(&workspace.root, &mut cache).unwrap_err();

        assert_eq!(error, CoreError::DestinationPageExists);
        assert_eq!(workspace.read_file("A___B.md"), "- body\n");
        assert_eq!(workspace.read_file("A___C.md"), "- external\n");
        assert!(workspace.root.join(".uniseq-page-transaction").exists());
    }
}

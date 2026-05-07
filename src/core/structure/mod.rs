mod planning;
mod transaction;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, PageId, PageLocation, PageName, WorkspaceCache, resolve_workspace_path};
use crate::core::discovery::materialize_parent_pages;
use crate::core::files::{load_workspace_cache, page_from_markdown_in_location};

use planning::{RenameTransactionPlan, moved_page_id, page_id_has_prefix, plan_transaction, renamed_page_id};
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
            changed_page_ids: plan
                .file_changes
                .iter()
                .map(|change| PageId::from_workspace_path(&change.final_path))
                .collect::<Result<Vec<_>, _>>()
                .expect("transaction plans only contain valid workspace paths"),
            removed_page_ids: plan
                .deletes
                .iter()
                .map(PageId::from_workspace_path)
                .collect::<Result<Vec<_>, _>>()
                .expect("transaction plans only contain valid workspace paths"),
        }
    }
}

impl OperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Move => "move",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "rename" => Some(Self::Rename),
            "move" => Some(Self::Move),
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
    let referrers = pages_referring_to_any(cache, ancestor_page_ids.iter().chain(std::iter::once(&request.page_id)))?;
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
    let relative_path = stream_location(&request.stream_name).workspace_path_for_page_id(&page_id)?;
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

    let disk_cache = load_workspace_cache(root)?;
    let Some(page) = disk_cache.page(&request.page_id) else {
        return Err(CoreError::MissingPage);
    };
    if page.location.is_stream_backed() {
        return Err(CoreError::UnsupportedStreamOperation {
            operation: "delete_subtree",
        });
    }

    let deleted_pages = disk_cache
        .pages()
        .values()
        .filter(|page| page.location.is_page_backed() && page_id_has_prefix(&page.page_id, &request.page_id))
        .cloned()
        .collect::<Vec<_>>();
    if deleted_pages.is_empty() {
        return Err(CoreError::MissingPage);
    }

    let deleted_page_ids = deleted_pages
        .iter()
        .map(|page| page.page_id.clone())
        .collect::<Vec<_>>();
    let deleted_set = deleted_page_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut changed_page_ids = pages_referring_to_any(cache, deleted_page_ids.iter())?
        .into_iter()
        .filter(|page_id| !deleted_set.contains(page_id))
        .collect::<Vec<_>>();
    if let Some(parent_page_id) = request.page_id.parent() {
        if !deleted_set.contains(&parent_page_id) && cache.page(&parent_page_id).is_some() {
            changed_page_ids.push(parent_page_id);
        }
    }

    let mut deleted_paths = Vec::new();
    for page in &deleted_pages {
        let absolute_path = root.join(&page.workspace_path);
        match fs::remove_file(&absolute_path) {
            Ok(()) => deleted_paths.push(page.workspace_path.clone()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CoreError::io(&absolute_path, &error)),
        }
    }

    for page_id in &deleted_page_ids {
        cache.remove_page(page_id);
    }

    changed_page_ids.sort();
    changed_page_ids.dedup();

    Ok(IncrementalWorkspaceUpdate {
        written_paths: Vec::new(),
        deleted_paths,
        changed_page_ids,
        removed_page_ids: deleted_page_ids,
    })
}

pub(crate) fn apply_stream_page_delete_with_update(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: StreamPageDelete,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let page_id = stream_page_id(&request.stream_name, &request.date_name)?;
    let disk_cache = load_workspace_cache(root)?;
    let Some(page) = disk_cache.page(&page_id) else {
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
    complete_transaction_record(root, cache, record, None)?;
    Ok(true)
}

pub(crate) fn transaction_record_exists(root: impl AsRef<Path>) -> bool {
    TransactionRecord::exists(root.as_ref())
}

pub(crate) fn is_transaction_relative_path(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_os_str() == ".uniseq-page-transaction")
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
    complete_transaction_record(root, cache, record, Some(&plan))
}

fn complete_transaction_record(
    root: &Path,
    cache: &mut WorkspaceCache,
    mut record: TransactionRecord,
    prepared_plan: Option<&RenameTransactionPlan>,
) -> Result<IncrementalWorkspaceUpdate, CoreError> {
    // Recovery intentionally finishes the rename/move to its planned final state
    // rather than attempting rollback. Markdown files remain authoritative, and
    // startup/runtime recovery replays the recorded final state until disk and
    // cache converge on one deterministic committed outcome.
    record.validate_final_paths_available(root)?;
    record.mark_applying(root)?;
    record.apply_final_state(root, None, false)?;
    let update = if let Some(plan) = prepared_plan {
        apply_transaction_plan_to_cache(cache, plan)?;
        IncrementalWorkspaceUpdate::from_plan(plan)
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

#[cfg(test)]
pub(crate) fn stage_page_rename_transaction_for_testing(
    root: impl AsRef<Path>,
    source_page_id: &PageId,
    new_leaf_name: &PageName,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    let target_page_id = renamed_page_id(source_page_id, new_leaf_name)?;
    let disk_cache = load_workspace_cache(root)?;
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
    let target_page_ids = target_page_ids.into_iter().cloned().collect::<BTreeSet<_>>();
    let mut source_page_ids = BTreeSet::new();

    for page in cache.pages().values() {
        for outgoing_ref in page.outgoing_refs() {
            if target_page_ids.contains(&outgoing_ref.target_page_id) {
                source_page_ids.insert(page.page_id.clone());
                break;
            }
        }
    }

    Ok(source_page_ids.into_iter().collect())
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
                .page(&PageId::new(["A", "Renamed"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
        assert_eq!(
            cache
                .page(&PageId::new(["A", "Renamed", "C"]).unwrap())
                .unwrap()
                .incoming_refs
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
                date_name: PageName::new("2026-05-07").unwrap(),
            },
        )
        .unwrap();

        assert!(workspace.file_exists("streams/journal/2026-05-07.md"));
        assert!(cache
            .page(
                &PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026-05-07").unwrap(),
                )
                .unwrap(),
            )
            .is_some());

        apply_stream_page_delete(
            &workspace.root,
            &mut cache,
            StreamPageDelete {
                stream_name: PageName::new("journal").unwrap(),
                date_name: PageName::new("2026-05-07").unwrap(),
            },
        )
        .unwrap();

        assert!(!workspace.file_exists("streams/journal/2026-05-07.md"));
        assert!(cache
            .page(
                &PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026-05-07").unwrap(),
                )
                .unwrap(),
            )
            .is_none());
    }

    #[test]
    fn rename_and_move_reject_stream_pages() {
        let workspace = TestWorkspace::new("uniseq-structure");
        workspace.write_file("streams/journal/2026-05-07.md", "- body\n");

        let mut cache = discover_workspace(&workspace.root).unwrap().cache;

        let rename_error = apply_page_rename(
            &workspace.root,
            &mut cache,
            PageRename {
                source_page_id: PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026-05-07").unwrap(),
                )
                .unwrap(),
                new_leaf_name: PageName::new("2026-05-08").unwrap(),
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
                    PageName::new("2026-05-07").unwrap(),
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

        let read_api = WorkspaceReadApi::new(&cache);
        let x_blocks = read_api.page_blocks(&PageId::new(["X"]).unwrap()).unwrap();
        assert_eq!(x_blocks.blocks[0].outgoing_refs.len(), 2);
        assert!(!x_blocks.blocks[0].outgoing_refs[0].target_exists);
        assert!(!x_blocks.blocks[0].outgoing_refs[1].target_exists);
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

        let read_api = WorkspaceReadApi::new(&cache);
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
        workspace.write_file("A___B.md", "- [[A/B/C]]\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("X.md", "- [[A/B]] and #A/B/C\n");

        let disk_cache = load_workspace_cache(&workspace.root).unwrap();
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

        assert_eq!(workspace.read_file("A___Renamed.md"), "- [[A/Renamed/C]]\n");
        assert_eq!(workspace.read_file("A___Renamed___C.md"), "- child\n");
        assert_eq!(
            workspace.read_file("X.md"),
            "- [[A/Renamed]] and #A/Renamed/C\n"
        );
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

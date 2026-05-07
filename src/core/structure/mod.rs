mod planning;
mod transaction;

use std::fs;
use std::path::Path;

use super::{CoreError, PageId, PageName, WorkspaceCache};
use crate::core::discovery::materialize_parent_pages;
use crate::core::files::load_workspace_cache;
use crate::core::files::refresh_workspace_cache;

use planning::{moved_page_id, page_id_has_prefix, plan_transaction, renamed_page_id};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationKind {
    Rename,
    Move,
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
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let relative_path = request.page_id.to_workspace_path();
    let absolute_path = root.join(&relative_path);
    if cache.page(&request.page_id).is_some() || absolute_path.exists() {
        return Err(CoreError::DestinationPageExists);
    }

    materialize_parent_pages(root, cache, request.page_id.ancestors())?;
    fs::write(&absolute_path, "").map_err(|error| CoreError::io(&absolute_path, &error))?;
    refresh_workspace_cache(root, cache)?;
    Ok(())
}

pub fn apply_page_delete_subtree(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageDeleteSubtree,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let disk_cache = load_workspace_cache(root)?;
    if disk_cache.page(&request.page_id).is_none() {
        return Err(CoreError::MissingPage);
    }

    let mut deleted_any = false;
    for page_id in disk_cache
        .pages()
        .keys()
        .filter(|page_id| page_id_has_prefix(page_id, &request.page_id))
    {
        let absolute_path = root.join(page_id.to_workspace_path());
        match fs::remove_file(&absolute_path) {
            Ok(()) => deleted_any = true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CoreError::io(&absolute_path, &error)),
        }
    }

    if !deleted_any {
        return Err(CoreError::MissingPage);
    }

    refresh_workspace_cache(root, cache)?;
    Ok(())
}

pub fn apply_page_rename(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageRename,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

    let target_page_id = renamed_page_id(&request.source_page_id, &request.new_leaf_name)?;
    if target_page_id == request.source_page_id {
        return Ok(());
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

pub fn apply_page_move(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    request: PageMove,
) -> Result<(), CoreError> {
    let root = root.as_ref();
    recover_workspace_transactions(root, cache)?;

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
        return Ok(());
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
    complete_transaction_record(root, cache, record)?;
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
) -> Result<(), CoreError> {
    let disk_cache = load_workspace_cache(root)?;
    let plan = plan_transaction(
        &disk_cache,
        operation_kind,
        source_page_id,
        target_page_id,
        destination_parent_page_id,
    )?;
    let record = TransactionRecord::stage(root, &plan)?;
    complete_transaction_record(root, cache, record)
}

fn complete_transaction_record(
    root: &Path,
    cache: &mut WorkspaceCache,
    mut record: TransactionRecord,
) -> Result<(), CoreError> {
    // Recovery intentionally finishes the rename/move to its planned final state
    // rather than attempting rollback. Markdown files remain authoritative, and
    // startup/runtime recovery replays the recorded final state until disk and
    // cache converge on one deterministic committed outcome.
    record.validate_final_paths_available(root)?;
    record.mark_applying(root)?;
    record.apply_final_state(root, None, false)?;
    record.remove(root)?;
    refresh_workspace_cache(root, cache)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoreError, Page, WorkspaceReadApi, discover_workspace};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("uniseq-structure-{unique}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_file(&self, relative_path: &str, contents: &str) {
            fs::write(self.root.join(relative_path), contents).unwrap();
        }

        fn read_file(&self, relative_path: &str) -> String {
            fs::read_to_string(self.root.join(relative_path)).unwrap()
        }

        fn file_exists(&self, relative_path: &str) -> bool {
            self.root.join(relative_path).exists()
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn rename_moves_subtree_and_rewrites_inbound_refs() {
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();

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
        let workspace = TestWorkspace::new();
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
    fn delete_removes_page_subtree_and_refreshes_refs() {
        let workspace = TestWorkspace::new();
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
        assert_eq!(x_blocks[0].outgoing_refs.len(), 2);
        assert!(!x_blocks[0].outgoing_refs[0].target_exists);
        assert!(!x_blocks[0].outgoing_refs[1].target_exists);
    }

    #[test]
    fn delete_rejects_missing_pages() {
        let workspace = TestWorkspace::new();

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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
        let workspace = TestWorkspace::new();
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
    fn recovery_rejects_late_destination_collisions_before_commit() {
        let workspace = TestWorkspace::new();
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

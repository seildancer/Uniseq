use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

use super::{
    BlockHandle, BlockSnapshot, BlockSubtreeEdit, CoreError, LinkedRefEntry, PageDetail, PageId,
    PageSummary, WorkspaceCache, WorkspaceReadApi, apply_block_subtree_edit as write_block_subtree,
    apply_page_move as write_page_move, apply_page_rename as write_page_rename,
};
use crate::core::files::{
    load_page_from_relative_path, load_workspace_cache, refresh_workspace_cache,
};
use crate::core::rename::{
    PageMove, PageRename, recover_workspace_transactions, transaction_record_exists,
};

const DEFAULT_WATCH_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    RecoveryApplied,
    WorkspaceReloaded,
    PagesChanged { page_ids: Vec<PageId> },
    PageRemoved { page_id: PageId },
}

pub struct WorkspaceSession {
    state: Arc<Mutex<WorkspaceSessionState>>,
    watcher: Option<PollingWatcher>,
}

struct PollingWatcher {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

#[derive(Debug)]
struct WorkspaceSessionState {
    root: PathBuf,
    cache: WorkspaceCache,
    fs_snapshot: WorkspaceFsSnapshot,
    pending_events: Vec<WorkspaceEvent>,
    last_watch_error: Option<CoreError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceFsSnapshot {
    markdown_files: BTreeMap<PathBuf, FileStamp>,
    transaction_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len_bytes: u64,
    modified_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheDiff {
    changed_page_ids: Vec<PageId>,
    removed_page_ids: Vec<PageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IsolatedFsChange {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

impl WorkspaceSession {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref().to_path_buf();
        let mut cache = WorkspaceCache::new();
        let recovery_applied = recover_workspace_transactions(&root, &mut cache)?;
        if !recovery_applied {
            cache = load_workspace_cache(&root)?;
        }

        let fs_snapshot = WorkspaceFsSnapshot::capture(&root)?;
        let mut pending_events = Vec::new();
        if recovery_applied {
            pending_events.push(WorkspaceEvent::RecoveryApplied);
        }

        Ok(Self {
            state: Arc::new(Mutex::new(WorkspaceSessionState {
                root,
                cache,
                fs_snapshot,
                pending_events,
                last_watch_error: None,
            })),
            watcher: None,
        })
    }

    pub fn workspace_root(&self) -> PathBuf {
        self.state.lock().unwrap().root.clone()
    }

    pub fn all_pages(&self) -> Vec<PageSummary> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.all_pages())
    }

    pub fn page_summary(&self, page_id: &PageId) -> Result<PageSummary, CoreError> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.page_summary(page_id))
    }

    pub fn page_detail(&self, page_id: &PageId) -> Result<PageDetail, CoreError> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.page_detail(page_id))
    }

    pub fn page_blocks(&self, page_id: &PageId) -> Result<Vec<BlockSnapshot>, CoreError> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.page_blocks(page_id))
    }

    pub fn block_by_handle(&self, handle: &BlockHandle) -> Result<BlockSnapshot, CoreError> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.block_by_handle(handle))
    }

    pub fn linked_refs(&self, target_page_id: &PageId) -> Result<Vec<LinkedRefEntry>, CoreError> {
        self.state
            .lock()
            .unwrap()
            .with_read_api(|read_api| read_api.linked_refs(target_page_id))
    }

    pub fn apply_block_subtree_edit(&self, edit: BlockSubtreeEdit) -> Result<(), CoreError> {
        self.state
            .lock()
            .unwrap()
            .apply_write(|root, cache| write_block_subtree(root, cache, edit))
    }

    pub fn apply_page_rename(&self, request: PageRename) -> Result<(), CoreError> {
        self.state
            .lock()
            .unwrap()
            .apply_write(|root, cache| write_page_rename(root, cache, request))
    }

    pub fn apply_page_move(&self, request: PageMove) -> Result<(), CoreError> {
        self.state
            .lock()
            .unwrap()
            .apply_write(|root, cache| write_page_move(root, cache, request))
    }

    pub fn poll_once(&self) -> Result<(), CoreError> {
        self.state.lock().unwrap().poll_once()
    }

    pub fn drain_events(&self) -> Vec<WorkspaceEvent> {
        self.state.lock().unwrap().drain_events()
    }

    pub fn take_last_watch_error(&self) -> Option<CoreError> {
        self.state.lock().unwrap().last_watch_error.take()
    }

    pub fn start_watching(&mut self, poll_interval: Duration) {
        if self.watcher.is_some() {
            return;
        }

        let state = Arc::clone(&self.state);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_signal = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_signal.load(Ordering::Relaxed) {
                thread::sleep(poll_interval);
                if stop_signal.load(Ordering::Relaxed) {
                    break;
                }

                let mut state = state.lock().unwrap();
                if let Err(error) = state.poll_once() {
                    state.last_watch_error = Some(error);
                }
            }
        });

        self.watcher = Some(PollingWatcher { stop, handle });
    }

    pub fn start_watching_default(&mut self) {
        self.start_watching(DEFAULT_WATCH_POLL_INTERVAL);
    }

    pub fn stop_watching(&mut self) {
        let Some(watcher) = self.watcher.take() else {
            return;
        };

        watcher.stop.store(true, Ordering::Relaxed);
        let _ = watcher.handle.join();
    }
}

impl Drop for WorkspaceSession {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

impl WorkspaceSessionState {
    fn with_read_api<R>(&self, f: impl FnOnce(WorkspaceReadApi<'_>) -> R) -> R {
        f(WorkspaceReadApi::new(&self.cache))
    }

    fn drain_events(&mut self) -> Vec<WorkspaceEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn poll_once(&mut self) -> Result<(), CoreError> {
        let snapshot = WorkspaceFsSnapshot::capture(&self.root)?;
        if snapshot == self.fs_snapshot {
            self.last_watch_error = None;
            return Ok(());
        }

        if snapshot.transaction_exists || self.fs_snapshot.transaction_exists {
            return self.full_refresh(true);
        }

        match classify_isolated_fs_change(&self.fs_snapshot, &snapshot) {
            Some(change) => self.apply_isolated_fs_change(snapshot, change),
            None => self.full_refresh(false),
        }
    }

    fn apply_write(
        &mut self,
        write: impl FnOnce(&Path, &mut WorkspaceCache) -> Result<(), CoreError>,
    ) -> Result<(), CoreError> {
        let old_cache = self.cache.clone();
        write(&self.root, &mut self.cache)?;
        self.fs_snapshot = WorkspaceFsSnapshot::capture(&self.root)?;
        self.last_watch_error = None;
        self.pending_events
            .extend(cache_diff(&old_cache, &self.cache).into_events());
        Ok(())
    }

    fn apply_isolated_fs_change(
        &mut self,
        snapshot: WorkspaceFsSnapshot,
        change: IsolatedFsChange,
    ) -> Result<(), CoreError> {
        let old_cache = self.cache.clone();
        let refresh_result = match change {
            IsolatedFsChange::Created(relative_path)
            | IsolatedFsChange::Modified(relative_path) => {
                let page = load_page_from_relative_path(&self.root, &relative_path)?;
                self.cache.upsert_page(page);
                Ok(())
            }
            IsolatedFsChange::Deleted(relative_path) => {
                let page_id = PageId::from_workspace_path(&relative_path)?;
                self.cache.remove_page(&page_id);
                Ok(())
            }
        };

        match refresh_result {
            Ok(()) => {
                self.fs_snapshot = snapshot;
                self.last_watch_error = None;
                self.pending_events
                    .extend(cache_diff(&old_cache, &self.cache).into_events());
                Ok(())
            }
            Err(CoreError::InvalidPagePath(_)) | Err(CoreError::Io { .. }) => {
                self.full_refresh(false)
            }
            Err(error) => Err(error),
        }
    }

    fn full_refresh(&mut self, may_need_recovery: bool) -> Result<(), CoreError> {
        let old_cache = self.cache.clone();
        let recovery_applied = if may_need_recovery || transaction_record_exists(&self.root) {
            recover_workspace_transactions(&self.root, &mut self.cache)?
        } else {
            false
        };

        if !recovery_applied {
            refresh_workspace_cache(&self.root, &mut self.cache)?;
        }

        self.fs_snapshot = WorkspaceFsSnapshot::capture(&self.root)?;
        self.last_watch_error = None;
        if recovery_applied {
            self.pending_events.push(WorkspaceEvent::RecoveryApplied);
        }
        self.pending_events.push(WorkspaceEvent::WorkspaceReloaded);

        let diff = cache_diff(&old_cache, &self.cache);
        self.pending_events.extend(diff.into_events());
        Ok(())
    }
}

impl WorkspaceFsSnapshot {
    fn capture(root: &Path) -> Result<Self, CoreError> {
        let mut markdown_files = BTreeMap::new();
        collect_workspace_snapshot(root, root, &mut markdown_files)?;
        Ok(Self {
            markdown_files,
            transaction_exists: transaction_record_exists(root),
        })
    }
}

impl CacheDiff {
    fn into_events(self) -> Vec<WorkspaceEvent> {
        let mut events = Vec::new();
        if !self.changed_page_ids.is_empty() {
            events.push(WorkspaceEvent::PagesChanged {
                page_ids: self.changed_page_ids,
            });
        }
        events.extend(
            self.removed_page_ids
                .into_iter()
                .map(|page_id| WorkspaceEvent::PageRemoved { page_id }),
        );
        events
    }
}

fn cache_diff(old_cache: &WorkspaceCache, new_cache: &WorkspaceCache) -> CacheDiff {
    let mut changed_page_ids = BTreeSet::new();
    let mut removed_page_ids = Vec::new();

    for (page_id, old_page) in old_cache.pages() {
        match new_cache.page(page_id) {
            Some(new_page) if new_page == old_page => {}
            Some(_) => {
                changed_page_ids.insert(page_id.clone());
            }
            None => removed_page_ids.push(page_id.clone()),
        }
    }

    for (page_id, new_page) in new_cache.pages() {
        if old_cache
            .page(page_id)
            .is_none_or(|old_page| old_page != new_page)
        {
            changed_page_ids.insert(page_id.clone());
        }
    }

    CacheDiff {
        changed_page_ids: changed_page_ids.into_iter().collect(),
        removed_page_ids,
    }
}

fn classify_isolated_fs_change(
    old_snapshot: &WorkspaceFsSnapshot,
    new_snapshot: &WorkspaceFsSnapshot,
) -> Option<IsolatedFsChange> {
    let mut created = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for (path, old_stamp) in &old_snapshot.markdown_files {
        match new_snapshot.markdown_files.get(path) {
            Some(new_stamp) if new_stamp == old_stamp => {}
            Some(_) => modified.push(path.clone()),
            None => deleted.push(path.clone()),
        }
    }

    for path in new_snapshot.markdown_files.keys() {
        if !old_snapshot.markdown_files.contains_key(path) {
            created.push(path.clone());
        }
    }

    match created.len() + modified.len() + deleted.len() {
        1 if created.len() == 1 => Some(IsolatedFsChange::Created(created.pop().unwrap())),
        1 if modified.len() == 1 => Some(IsolatedFsChange::Modified(modified.pop().unwrap())),
        1 if deleted.len() == 1 => Some(IsolatedFsChange::Deleted(deleted.pop().unwrap())),
        _ => None,
    }
}

fn collect_workspace_snapshot(
    root: &Path,
    current_dir: &Path,
    markdown_files: &mut BTreeMap<PathBuf, FileStamp>,
) -> Result<(), CoreError> {
    let mut entries =
        fs::read_dir(current_dir).map_err(|error| CoreError::io(current_dir, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(current_dir, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;

        if file_type.is_dir() {
            collect_workspace_snapshot(root, &entry_path, markdown_files)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let is_markdown = entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
        if !is_markdown {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(root)
            .map_err(|_| {
                CoreError::io(
                    root,
                    &std::io::Error::from(std::io::ErrorKind::InvalidInput),
                )
            })?
            .to_path_buf();
        let metadata = entry
            .metadata()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;
        markdown_files.insert(
            relative_path,
            FileStamp {
                len_bytes: metadata.len(),
                modified_at: metadata
                    .modified()
                    .map_err(|error| CoreError::io(entry_path, &error))?,
            },
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileFingerprint, PageName, SourceSpan, WorkspaceReadApi,
        core::rename::stage_page_rename_transaction_for_testing,
    };
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
            let root = std::env::temp_dir().join(format!("uniseq-session-{unique}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_file(&self, relative_path: &str, contents: &str) {
            fs::write(self.root.join(relative_path), contents).unwrap();
        }

        fn remove_file(&self, relative_path: &str) {
            fs::remove_file(self.root.join(relative_path)).unwrap();
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn startup_recovers_interrupted_transactions_before_reads() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("X.md", "- [[A/B]]\n");
        stage_page_rename_transaction_for_testing(
            &workspace.root,
            &PageId::new(["A", "B"]).unwrap(),
            &PageName::new("C").unwrap(),
        )
        .unwrap();

        let session = WorkspaceSession::open(&workspace.root).unwrap();

        assert!(
            session
                .page_summary(&PageId::new(["A", "C"]).unwrap())
                .is_ok()
        );
        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::RecoveryApplied]
        );
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn poll_once_refreshes_single_changed_page_and_targets() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- [[C]]\n");
        session.poll_once().unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![
                    PageId::new(["A"]).unwrap(),
                    PageId::new(["B"]).unwrap(),
                    PageId::new(["C"]).unwrap(),
                ],
            }]
        );
        assert_eq!(
            session
                .page_detail(&PageId::new(["C"]).unwrap())
                .unwrap()
                .linked_ref_count,
            1
        );
    }

    #[test]
    fn poll_once_adds_created_pages_incrementally() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("B.md", "- body\n");
        session.poll_once().unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![PageId::new(["B"]).unwrap()],
            }]
        );
        assert_eq!(session.all_pages().len(), 2);
    }

    #[test]
    fn poll_once_removes_deleted_pages_and_rebuilds_refs() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.remove_file("A.md");
        session.poll_once().unwrap();

        assert_eq!(
            session.drain_events(),
            vec![
                WorkspaceEvent::PagesChanged {
                    page_ids: vec![PageId::new(["B"]).unwrap()],
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A"]).unwrap(),
                },
            ]
        );
        assert_eq!(
            session
                .page_detail(&PageId::new(["B"]).unwrap())
                .unwrap()
                .linked_ref_count,
            0
        );
    }

    #[test]
    fn multi_file_bursts_fall_back_to_workspace_reload() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "");
        workspace.write_file("B.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- changed\n");
        workspace.write_file("B.md", "- changed\n");
        session.poll_once().unwrap();

        let events = session.drain_events();
        assert!(events.contains(&WorkspaceEvent::WorkspaceReloaded));
    }

    #[test]
    fn session_writes_emit_invalidation_events_and_refresh_cache() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        let cache = load_workspace_cache(&workspace.root).unwrap();
        let read_api = WorkspaceReadApi::new(&cache);
        let handle = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap()[0]
            .handle
            .clone();
        session.drain_events();

        session
            .apply_block_subtree_edit(BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- [[C]]\n".to_owned(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![
                    PageId::new(["A"]).unwrap(),
                    PageId::new(["B"]).unwrap(),
                    PageId::new(["C"]).unwrap(),
                ],
            }]
        );
        assert_eq!(
            session
                .page_detail(&PageId::new(["C"]).unwrap())
                .unwrap()
                .linked_ref_count,
            1
        );
    }

    #[test]
    fn poll_once_is_quiet_after_backend_write_refreshes_snapshot() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- old\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        let page = load_workspace_cache(&workspace.root)
            .unwrap()
            .page(&PageId::new(["A"]).unwrap())
            .unwrap()
            .clone();
        let handle = BlockHandle::new(
            PageId::new(["A"]).unwrap(),
            page.fingerprint,
            SourceSpan::unchecked(0, page.text.len()),
        );
        session.drain_events();

        session
            .apply_block_subtree_edit(BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- new\n".to_owned(),
            })
            .unwrap();
        session.drain_events();
        session.poll_once().unwrap();

        assert!(session.drain_events().is_empty());
    }

    #[test]
    fn background_watcher_reports_errors_without_panicking() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "");
        let mut session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();
        session.start_watching(Duration::from_millis(25));

        fs::create_dir_all(workspace.root.join("nested")).unwrap();
        workspace.write_file("nested\\A.md", "");
        thread::sleep(Duration::from_millis(100));
        session.stop_watching();

        assert!(matches!(
            session.take_last_watch_error(),
            Some(CoreError::InvalidPagePath(_))
        ));
    }

    #[test]
    fn session_queries_use_current_cache_state() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- body\r\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();

        let block = session.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();
        assert_eq!(block[0].content, "body");
        assert_eq!(
            session
                .page_summary(&PageId::new(["A"]).unwrap())
                .unwrap()
                .page_id,
            PageId::new(["A"]).unwrap()
        );
        let cache = load_workspace_cache(&workspace.root).unwrap();
        assert_eq!(
            cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .fingerprint,
            FileFingerprint::from_text("- body\r\n")
        );
    }
}

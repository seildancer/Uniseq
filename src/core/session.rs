use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, UNIX_EPOCH};

use notify::{Config as NotifyConfig, Event, EventKind, RecursiveMode, Watcher};

use super::{
    CoreError, IncomingPageRefSnapshot, OutgoingPageRefSnapshot, PageBlocksSnapshot, PageDetail,
    PageId, PageSummary, WorkspaceCache, WorkspaceReadApi, supported_workspace_markdown_path,
};
use crate::core::files::{
    collect_supported_workspace_markdown_paths, load_page_with_fingerprint_from_relative_path,
    load_workspace_cache,
};
use crate::core::structure::{
    IncrementalWorkspaceUpdate, PageCreate, PageDeleteSubtree, PageMove, PageRename,
    StreamPageCreate, StreamPageDelete,
    apply_page_create_with_update, apply_page_delete_subtree_with_update, apply_page_move_with_update,
    apply_page_rename_with_update, apply_stream_page_create_with_update,
    apply_stream_page_delete_with_update, is_transaction_relative_path, recover_workspace_transactions,
    transaction_record_exists,
};

const DEFAULT_WATCH_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    RecoveryApplied,
    // Advisory coarse invalidation for runtime consumers. Frontends should treat
    // page-level events as the authoritative selective refresh signal and may
    // use WorkspaceReloaded only for broad caches or diagnostics.
    WorkspaceReloaded,
    PagesChanged { page_ids: Vec<PageId> },
    PageRemoved { page_id: PageId },
    WatcherModeChanged { mode: WatcherMode },
    WatcherDegradedToPolling { reason: WatcherFallbackReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherMode {
    Native,
    Polling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherFallbackReason {
    NativeWatcherSetupFailed { message: String },
    NativeWatcherRuntimeFailed { message: String },
    ControlChannelDisconnected,
}

pub struct WorkspaceSession {
    state: Arc<RwLock<WorkspaceSessionState>>,
    watcher: Option<WatcherHandle>,
}

struct WatcherHandle {
    stop: Sender<WatchLoopMessage>,
    handle: JoinHandle<()>,
}

#[derive(Debug)]
struct WorkspaceSessionState {
    root: PathBuf,
    cache: WorkspaceCache,
    fs_snapshot: WorkspaceFsSnapshot,
    pending_events: Vec<WorkspaceEvent>,
    last_watch_error: Option<CoreError>,
    watcher_mode: Option<WatcherMode>,
    watcher_fallback_reason: Option<WatcherFallbackReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceFsSnapshot {
    markdown_files: BTreeMap<PathBuf, FileStamp>,
    transaction_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len_bytes: u64,
    modified_at_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheDiff {
    changed_page_ids: Vec<PageId>,
    removed_page_ids: Vec<PageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageEventState {
    fingerprint: super::FileFingerprint,
    child_page_ids: Vec<PageId>,
    incoming_refs: Vec<super::IncomingRef>,
    outgoing_refs: Vec<OutgoingRefEventState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutgoingRefEventState {
    target_page_id: PageId,
    ref_span: super::SourceSpan,
    target_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct IncrementalFsUpdate {
    written_paths: BTreeSet<PathBuf>,
    deleted_paths: BTreeSet<PathBuf>,
}

#[derive(Debug)]
struct PreparedFullRefresh {
    cache: WorkspaceCache,
    fs_snapshot: WorkspaceFsSnapshot,
    recovery_applied: bool,
}

#[derive(Debug)]
struct PreparedIncrementalFsUpdate {
    deleted_paths: Vec<PathBuf>,
    deleted_page_ids: Vec<PageId>,
    written_files: Vec<PreparedWrittenFile>,
}

#[derive(Debug)]
struct PreparedWrittenFile {
    relative_path: PathBuf,
    page: crate::core::Page,
    file_stamp: FileStamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NativeEventAction {
    Noop,
    TransactionPathChanged,
    IncrementalPaths(BTreeSet<PathBuf>),
    FallbackToSnapshot,
}

enum WatchLoopMessage {
    Fs(notify::Result<notify::Event>),
    Stop,
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
            state: Arc::new(RwLock::new(WorkspaceSessionState {
                root,
                cache,
                fs_snapshot,
                pending_events,
                last_watch_error: None,
                watcher_mode: None,
                watcher_fallback_reason: None,
            })),
            watcher: None,
        })
    }

    pub fn workspace_root(&self) -> PathBuf {
        self.state.read().unwrap().root.clone()
    }

    pub fn all_pages(&self) -> Vec<PageSummary> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.all_pages())
    }

    pub fn page_summary(&self, page_id: &PageId) -> Result<PageSummary, CoreError> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.page_summary(page_id))
    }

    pub fn page_detail(&self, page_id: &PageId) -> Result<PageDetail, CoreError> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.page_detail(page_id))
    }

    pub fn page_blocks(&self, page_id: &PageId) -> Result<PageBlocksSnapshot, CoreError> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.page_blocks(page_id))
    }

    pub fn page_incoming_refs(
        &self,
        target_page_id: &PageId,
    ) -> Result<Vec<IncomingPageRefSnapshot>, CoreError> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.page_incoming_refs(target_page_id))
    }

    pub fn page_outgoing_refs(
        &self,
        source_page_id: &PageId,
    ) -> Result<Vec<OutgoingPageRefSnapshot>, CoreError> {
        self.state
            .read()
            .unwrap()
            .with_read_api(|read_api| read_api.page_outgoing_refs(source_page_id))
    }

    pub fn apply_page_create(&self, request: PageCreate) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| apply_page_create_with_update(root, cache, request))
    }

    pub fn apply_page_delete_subtree(&self, request: PageDeleteSubtree) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| {
                apply_page_delete_subtree_with_update(root, cache, request)
            })
    }

    pub fn apply_stream_page_create(&self, request: StreamPageCreate) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| {
                apply_stream_page_create_with_update(root, cache, request)
            })
    }

    pub fn apply_stream_page_delete(&self, request: StreamPageDelete) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| {
                apply_stream_page_delete_with_update(root, cache, request)
            })
    }

    pub fn apply_page_rename(&self, request: PageRename) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| apply_page_rename_with_update(root, cache, request))
    }

    pub fn apply_page_move(&self, request: PageMove) -> Result<(), CoreError> {
        self.state
            .write()
            .unwrap()
            .apply_incremental_write(|root, cache| apply_page_move_with_update(root, cache, request))
    }

    pub fn poll_once(&self) -> Result<(), CoreError> {
        let (root, old_snapshot) = {
            let state = self.state.read().unwrap();
            (state.root.clone(), state.fs_snapshot.clone())
        };
        let snapshot = WorkspaceFsSnapshot::capture(&root)?;

        if snapshot == old_snapshot {
            self.state.write().unwrap().last_watch_error = None;
            return Ok(());
        }

        if snapshot.transaction_exists || old_snapshot.transaction_exists {
            let prepared = prepare_full_refresh(&root, true)?;
            let mut state = self.state.write().unwrap();
            if state.fs_snapshot != old_snapshot {
                return state.poll_once();
            }
            return state.apply_prepared_full_refresh(prepared);
        }

        let update = classify_snapshot_fs_changes(&old_snapshot, &snapshot);
        let prepared = prepare_incremental_fs_update(&root, &update)?;
        let mut state = self.state.write().unwrap();
        if state.fs_snapshot != old_snapshot {
            return state.poll_once();
        }
        state.apply_prepared_incremental_fs_update(snapshot, prepared)
    }

    pub fn drain_events(&self) -> Vec<WorkspaceEvent> {
        self.state.write().unwrap().drain_events()
    }

    pub fn take_last_watch_error(&self) -> Option<CoreError> {
        self.state.write().unwrap().last_watch_error.take()
    }

    pub fn watcher_mode(&self) -> Option<WatcherMode> {
        self.state.read().unwrap().watcher_mode
    }

    pub fn watcher_fallback_reason(&self) -> Option<WatcherFallbackReason> {
        self.state.read().unwrap().watcher_fallback_reason.clone()
    }

    pub fn start_watching(&mut self, poll_interval: Duration) {
        if self.watcher.is_some() {
            return;
        }

        let state = Arc::clone(&self.state);
        let (tx, rx) = mpsc::channel();
        let stop = tx.clone();
        let handle = thread::spawn(move || {
            run_native_or_polling_watch_loop(state, rx, tx, poll_interval);
        });

        self.watcher = Some(WatcherHandle { stop, handle });
    }

    pub fn start_watching_default(&mut self) {
        self.start_watching(DEFAULT_WATCH_POLL_INTERVAL);
    }

    pub fn stop_watching(&mut self) {
        let Some(watcher) = self.watcher.take() else {
            return;
        };

        let _ = watcher.stop.send(WatchLoopMessage::Stop);
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

    fn enqueue_event(&mut self, event: WorkspaceEvent) {
        match event {
            WorkspaceEvent::PagesChanged { mut page_ids } => {
                page_ids.sort();
                page_ids.dedup();

                if let Some(WorkspaceEvent::PagesChanged {
                    page_ids: existing_page_ids,
                }) = self
                    .pending_events
                    .iter_mut()
                    .find(|existing| matches!(existing, WorkspaceEvent::PagesChanged { .. }))
                {
                    existing_page_ids.extend(page_ids);
                    existing_page_ids.sort();
                    existing_page_ids.dedup();
                } else if !page_ids.is_empty() {
                    self.pending_events
                        .push(WorkspaceEvent::PagesChanged { page_ids });
                }
            }
            WorkspaceEvent::WatcherModeChanged { mode } => {
                self.pending_events
                    .retain(|event| !matches!(event, WorkspaceEvent::WatcherModeChanged { .. }));
                self.pending_events
                    .push(WorkspaceEvent::WatcherModeChanged { mode });
            }
            WorkspaceEvent::WatcherDegradedToPolling { reason } => {
                self.pending_events.retain(
                    |event| !matches!(event, WorkspaceEvent::WatcherDegradedToPolling { .. }),
                );
                self.pending_events
                    .push(WorkspaceEvent::WatcherDegradedToPolling { reason });
            }
            WorkspaceEvent::RecoveryApplied => {
                if !self
                    .pending_events
                    .iter()
                    .any(|event| matches!(event, WorkspaceEvent::RecoveryApplied))
                {
                    self.pending_events.push(WorkspaceEvent::RecoveryApplied);
                }
            }
            WorkspaceEvent::WorkspaceReloaded => {
                if !self
                    .pending_events
                    .iter()
                    .any(|event| matches!(event, WorkspaceEvent::WorkspaceReloaded))
                {
                    self.pending_events.push(WorkspaceEvent::WorkspaceReloaded);
                }
            }
            WorkspaceEvent::PageRemoved { page_id } => {
                if !self.pending_events.iter().any(|event| {
                    matches!(event, WorkspaceEvent::PageRemoved { page_id: existing } if existing == &page_id)
                }) {
                    self.pending_events.push(WorkspaceEvent::PageRemoved { page_id });
                }
            }
        }
    }

    fn enqueue_events(&mut self, events: impl IntoIterator<Item = WorkspaceEvent>) {
        for event in events {
            self.enqueue_event(event);
        }
    }

    fn record_watcher_mode(&mut self, mode: WatcherMode) {
        if self.watcher_mode == Some(mode) {
            return;
        }

        self.watcher_mode = Some(mode);
        self.enqueue_event(WorkspaceEvent::WatcherModeChanged { mode });
    }

    fn record_watcher_degraded_to_polling(&mut self, reason: WatcherFallbackReason) {
        println!(
            "[uniseq-backend] watcher fallback: degrading to polling mode: {:?}",
            reason
        );
        self.watcher_fallback_reason = Some(reason.clone());
        self.enqueue_event(WorkspaceEvent::WatcherDegradedToPolling { reason });
        self.record_watcher_mode(WatcherMode::Polling);
    }

    fn page_event_states(&self) -> BTreeMap<PageId, PageEventState> {
        self.cache
            .pages()
            .iter()
            .map(|(page_id, page)| {
                (
                    page_id.clone(),
                    PageEventState {
                        fingerprint: page.fingerprint,
                        child_page_ids: page.child_page_ids.clone(),
                        incoming_refs: self
                            .cache
                            .incoming_refs(page_id)
                            .to_vec(),
                        outgoing_refs: page
                            .outgoing_refs()
                            .map(|page_ref| OutgoingRefEventState {
                                target_page_id: page_ref.target_page_id.clone(),
                                ref_span: page_ref.ref_span,
                                target_exists: self.cache.page(&page_ref.target_page_id).is_some(),
                            })
                            .collect(),
                    },
                )
            })
            .collect()
    }

    fn emit_cache_diff(&mut self, old_states: BTreeMap<PageId, PageEventState>) {
        self.enqueue_events(
            cache_diff_from_states(&old_states, &self.page_event_states()).into_events(),
        );
    }

    fn emit_incremental_update_events(&mut self, update: &IncrementalWorkspaceUpdate) {
        if !update.changed_page_ids.is_empty() {
            self.enqueue_event(WorkspaceEvent::PagesChanged {
                page_ids: update.changed_page_ids.clone(),
            });
        }

        for page_id in &update.removed_page_ids {
            self.enqueue_event(WorkspaceEvent::PageRemoved {
                page_id: page_id.clone(),
            });
        }
    }

    fn poll_once(&mut self) -> Result<(), CoreError> {
        println!(
            "[uniseq-backend] supported-root scan: capturing polling snapshot at {}",
            self.root.display()
        );
        let snapshot = WorkspaceFsSnapshot::capture(&self.root)?;
        if snapshot == self.fs_snapshot {
            self.last_watch_error = None;
            return Ok(());
        }

        if snapshot.transaction_exists || self.fs_snapshot.transaction_exists {
            println!(
                "[uniseq-backend] whole-cache refresh: polling detected transaction state, triggering full refresh"
            );
            return self.full_refresh(true);
        }

        let update = classify_snapshot_fs_changes(&self.fs_snapshot, &snapshot);
        self.apply_incremental_fs_update(snapshot, update)
    }

    fn apply_incremental_write(
        &mut self,
        write: impl FnOnce(&Path, &mut WorkspaceCache) -> Result<IncrementalWorkspaceUpdate, CoreError>,
    ) -> Result<(), CoreError> {
        let update = write(&self.root, &mut self.cache)?;
        self.apply_incremental_snapshot_update(&update)?;
        self.emit_incremental_update_events(&update);
        self.last_watch_error = None;
        Ok(())
    }

    fn apply_incremental_fs_update(
        &mut self,
        snapshot: WorkspaceFsSnapshot,
        update: IncrementalFsUpdate,
    ) -> Result<(), CoreError> {
        let prepared = prepare_incremental_fs_update(&self.root, &update)?;
        self.apply_prepared_incremental_fs_update(snapshot, prepared)
    }

    fn apply_incremental_native_paths(
        &mut self,
        relative_paths: BTreeSet<PathBuf>,
    ) -> Result<(), CoreError> {
        let update = self.incremental_update_from_native_paths(relative_paths);
        let prepared = prepare_incremental_fs_update(&self.root, &update)?;
        let Some(cache_diff) = self.plan_incremental_cache_diff(&prepared) else {
            return self.full_refresh(false);
        };
        let deleted_paths = prepared.deleted_paths.clone();
        let written_snapshot_entries = prepared
            .written_files
            .iter()
            .map(|written_file| (written_file.relative_path.clone(), written_file.file_stamp))
            .collect::<Vec<_>>();
        match self.apply_prepared_incremental_fs_update_to_cache(prepared) {
            Ok(()) => {
                self.apply_snapshot_delta(&deleted_paths, written_snapshot_entries);
                self.last_watch_error = None;
                self.enqueue_events(cache_diff.into_events());
                Ok(())
            }
            Err(CoreError::InvalidPagePath(_)) | Err(CoreError::Io { .. }) => self.poll_once(),
            Err(error) => Err(error),
        }
    }

    fn apply_native_event_burst(&mut self, events: &[Event]) -> Result<(), CoreError> {
        match classify_native_event_burst(&self.root, events) {
            NativeEventAction::Noop => {
                self.last_watch_error = None;
                Ok(())
            }
            NativeEventAction::TransactionPathChanged => {
                println!(
                    "[uniseq-backend] native watcher fallback: transaction path changed, triggering full refresh"
                );
                self.full_refresh(true)
            }
            NativeEventAction::IncrementalPaths(relative_paths) => {
                self.apply_incremental_native_paths(relative_paths)
            }
            NativeEventAction::FallbackToSnapshot => {
                println!(
                    "[uniseq-backend] native watcher fallback: ambiguous event burst, switching to polling snapshot reconciliation"
                );
                self.poll_once()
            }
        }
    }

    fn full_refresh(&mut self, may_need_recovery: bool) -> Result<(), CoreError> {
        println!(
            "[uniseq-backend] whole-cache refresh: rebuilding cache from disk at {} (recovery={})",
            self.root.display(),
            may_need_recovery
        );
        let prepared = prepare_full_refresh(&self.root, may_need_recovery)?;
        self.apply_prepared_full_refresh(prepared)
    }

    fn apply_prepared_full_refresh(
        &mut self,
        prepared: PreparedFullRefresh,
    ) -> Result<(), CoreError> {
        let old_states = self.page_event_states();
        self.cache = prepared.cache;
        self.fs_snapshot = prepared.fs_snapshot;
        self.last_watch_error = None;
        if prepared.recovery_applied {
            self.enqueue_event(WorkspaceEvent::RecoveryApplied);
        }
        // WorkspaceReloaded is intentionally additive rather than authoritative:
        // callers still receive precise page-level invalidations from the cache
        // diff below and should prefer those for selective frontend refresh.
        self.enqueue_event(WorkspaceEvent::WorkspaceReloaded);

        self.emit_cache_diff(old_states);
        Ok(())
    }

    fn incremental_update_preserves_parent_pages(
        &self,
        deleted_page_ids: &[PageId],
        written_pages: &[crate::core::Page],
    ) -> bool {
        let deleted_page_ids = deleted_page_ids.iter().cloned().collect::<BTreeSet<_>>();
        let written_page_ids = written_pages
            .iter()
            .map(|page| page.page_id.clone())
            .collect::<BTreeSet<_>>();

        for page in written_pages {
            if !page.location.is_page_backed() {
                continue;
            }

            for ancestor_page_id in page.ancestor_page_ids() {
                let ancestor_survives_batch = written_page_ids.contains(&ancestor_page_id)
                    || (!deleted_page_ids.contains(&ancestor_page_id)
                        && self.cache.page(&ancestor_page_id).is_some());
                if !ancestor_survives_batch {
                    return false;
                }
            }
        }

        if deleted_page_ids.is_empty() {
            return true;
        }

        for page in self.cache.pages().values() {
            if deleted_page_ids.contains(&page.page_id) || written_page_ids.contains(&page.page_id) {
                continue;
            }

            if !page.location.is_page_backed() {
                continue;
            }

            if page
                .ancestor_page_ids()
                .iter()
                .any(|ancestor_page_id| deleted_page_ids.contains(ancestor_page_id))
            {
                return false;
            }
        }

        true
    }

    fn apply_incremental_page_update(&mut self, page: crate::core::Page) {
        if self.cache.page(&page.page_id).is_some() {
            self.cache.refresh_page_content(page);
        } else {
            self.cache.upsert_page(page);
        }
    }

    fn apply_snapshot_delta(
        &mut self,
        deleted_paths: &[PathBuf],
        written_files: impl IntoIterator<Item = (PathBuf, FileStamp)>,
    ) {
        for deleted_path in deleted_paths {
            self.fs_snapshot.markdown_files.remove(deleted_path);
        }

        for (written_path, file_stamp) in written_files {
            self.fs_snapshot.markdown_files.insert(written_path, file_stamp);
        }

        self.fs_snapshot.transaction_exists = false;
    }

    fn apply_prepared_incremental_fs_update(
        &mut self,
        snapshot: WorkspaceFsSnapshot,
        prepared: PreparedIncrementalFsUpdate,
    ) -> Result<(), CoreError> {
        let Some(cache_diff) = self.plan_incremental_cache_diff(&prepared) else {
            return self.full_refresh(false);
        };
        match self.apply_prepared_incremental_fs_update_to_cache(prepared) {
            Ok(()) => {
                self.fs_snapshot = snapshot;
                self.last_watch_error = None;
                self.enqueue_events(cache_diff.into_events());
                Ok(())
            }
            Err(CoreError::InvalidPagePath(_)) | Err(CoreError::Io { .. }) => {
                self.full_refresh(false)
            }
            Err(error) => Err(error),
        }
    }

    fn apply_prepared_incremental_fs_update_to_cache(
        &mut self,
        prepared: PreparedIncrementalFsUpdate,
    ) -> Result<(), CoreError> {
        for page_id in prepared.deleted_page_ids {
            self.cache.remove_page(&page_id);
        }

        for written_file in prepared.written_files {
            if self.written_file_changes_page(&written_file) {
                self.apply_incremental_page_update(written_file.page);
            }
        }

        Ok(())
    }

    fn plan_incremental_cache_diff(
        &self,
        prepared: &PreparedIncrementalFsUpdate,
    ) -> Option<CacheDiff> {
        let written_pages = prepared
            .written_files
            .iter()
            .filter(|written_file| self.written_file_changes_page(written_file))
            .map(|written_file| written_file.page.clone())
            .collect::<Vec<_>>();
        if !self.incremental_update_preserves_parent_pages(
            &prepared.deleted_page_ids,
            &written_pages,
        ) {
            return None;
        }

        let removed_page_ids = prepared
            .deleted_page_ids
            .iter()
            .filter(|page_id| self.cache.page(page_id).is_some())
            .cloned()
            .collect::<Vec<_>>();
        let removed_page_ids_set = removed_page_ids.iter().cloned().collect::<BTreeSet<_>>();
        let written_page_ids = prepared
            .written_files
            .iter()
            .map(|written_file| written_file.page.page_id.clone())
            .collect::<BTreeSet<_>>();
        let created_page_ids = written_page_ids
            .iter()
            .filter(|page_id| self.cache.page(page_id).is_none())
            .cloned()
            .collect::<BTreeSet<_>>();

        let mut changed_page_ids = BTreeSet::new();
        let existence_toggled_page_ids = removed_page_ids_set
            .union(&created_page_ids)
            .cloned()
            .collect::<BTreeSet<_>>();

        changed_page_ids.extend(
            self.page_ids_referring_to_any(&existence_toggled_page_ids)
                .into_iter()
                .filter(|page_id| !removed_page_ids_set.contains(page_id)),
        );

        for removed_page_id in &removed_page_ids {
            let Some(page) = self.cache.page(removed_page_id) else {
                continue;
            };

            if let Some(parent_page_id) = page.parent_page_id() {
                if !removed_page_ids_set.contains(&parent_page_id) {
                    changed_page_ids.insert(parent_page_id);
                }
            }

            changed_page_ids.extend(
                target_page_ids_from_page(page)
                    .into_iter()
                    .filter(|page_id| !removed_page_ids_set.contains(page_id)),
            );
        }

        for written_file in &prepared.written_files {
            let page = &written_file.page;
            changed_page_ids.insert(page.page_id.clone());

            if self.cache.page(&page.page_id).is_none() {
                if let Some(parent_page_id) = page.parent_page_id() {
                    changed_page_ids.insert(parent_page_id);
                }
            }

            if let Some(old_page) = self.cache.page(&page.page_id) {
                changed_page_ids.extend(
                    target_page_ids_from_page(old_page)
                        .into_iter()
                        .filter(|page_id| !removed_page_ids_set.contains(page_id)),
                );
            }

            changed_page_ids.extend(
                target_page_ids_from_page(page)
                    .into_iter()
                    .filter(|page_id| !removed_page_ids_set.contains(page_id)),
            );
        }

        Some(CacheDiff {
            changed_page_ids: changed_page_ids
                .into_iter()
                .filter(|page_id| !removed_page_ids_set.contains(page_id))
                .collect(),
            removed_page_ids,
        })
    }

    fn apply_incremental_snapshot_update(
        &mut self,
        update: &IncrementalWorkspaceUpdate,
    ) -> Result<(), CoreError> {
        let written_files = update
            .written_paths
            .iter()
            .map(|written_path| {
                let absolute_path = self.root.join(written_path);
                FileStamp::from_absolute_path(&absolute_path)
                    .map(|file_stamp| (written_path.clone(), file_stamp))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_snapshot_delta(&update.deleted_paths, written_files);
        Ok(())
    }

    fn incremental_update_from_native_paths(
        &self,
        relative_paths: BTreeSet<PathBuf>,
    ) -> IncrementalFsUpdate {
        let mut update = IncrementalFsUpdate::default();

        for relative_path in relative_paths {
            let absolute_path = self.root.join(&relative_path);
            if absolute_path.exists() {
                update.written_paths.insert(relative_path);
            } else {
                update.deleted_paths.insert(relative_path);
            }
        }

        update
    }

    fn page_ids_referring_to_any(&self, target_page_ids: &BTreeSet<PageId>) -> BTreeSet<PageId> {
        if target_page_ids.is_empty() {
            return BTreeSet::new();
        }

        target_page_ids
            .iter()
            .flat_map(|page_id| {
                self.cache
                    .incoming_refs(page_id)
                    .iter()
                    .map(|incoming_ref| incoming_ref.source_page_id.clone())
            })
            .collect()
    }

    fn written_file_changes_page(&self, written_file: &PreparedWrittenFile) -> bool {
        self.cache
            .page(&written_file.page.page_id)
            .is_none_or(|existing_page| {
                existing_page.workspace_path != written_file.relative_path
                    || existing_page.fingerprint != written_file.page.fingerprint
            })
    }
}

impl WorkspaceFsSnapshot {
    fn capture(root: &Path) -> Result<Self, CoreError> {
        println!(
            "[uniseq-backend] supported-root scan: capturing workspace snapshot at {}",
            root.display()
        );
        let mut markdown_files = BTreeMap::new();
        for relative_path in collect_supported_workspace_markdown_paths(root)? {
            let absolute_path = root.join(&relative_path);
            markdown_files.insert(relative_path, FileStamp::from_absolute_path(&absolute_path)?);
        }
        println!(
            "[uniseq-backend] supported-root scan complete: {} supported markdown files in snapshot",
            markdown_files.len()
        );
        Ok(Self {
            markdown_files,
            transaction_exists: transaction_record_exists(root),
        })
    }
}

impl FileStamp {
    fn from_absolute_path(absolute_path: &Path) -> Result<Self, CoreError> {
        let metadata = fs::metadata(absolute_path).map_err(|error| CoreError::io(absolute_path, &error))?;
        let modified_at_nanos = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(Self {
            len_bytes: metadata.len(),
            modified_at_nanos,
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

fn cache_diff_from_states(
    old_states: &BTreeMap<PageId, PageEventState>,
    new_states: &BTreeMap<PageId, PageEventState>,
) -> CacheDiff {
    let mut changed_page_ids = BTreeSet::new();
    let mut removed_page_ids = Vec::new();

    for (page_id, old_state) in old_states {
        match new_states.get(page_id) {
            Some(new_state) if new_state == old_state => {}
            Some(_) => {
                changed_page_ids.insert(page_id.clone());
            }
            None => removed_page_ids.push(page_id.clone()),
        }
    }

    for (page_id, new_state) in new_states {
        if old_states
            .get(page_id)
            .is_none_or(|old_state| old_state != new_state)
        {
            changed_page_ids.insert(page_id.clone());
        }
    }

    CacheDiff {
        changed_page_ids: changed_page_ids.into_iter().collect(),
        removed_page_ids,
    }
}

fn target_page_ids_from_page(page: &crate::core::Page) -> BTreeSet<PageId> {
    page.outgoing_refs()
        .map(|outgoing_ref| outgoing_ref.target_page_id.clone())
        .collect()
}

fn classify_snapshot_fs_changes(
    old_snapshot: &WorkspaceFsSnapshot,
    new_snapshot: &WorkspaceFsSnapshot,
) -> IncrementalFsUpdate {
    let mut update = IncrementalFsUpdate::default();

    for (path, old_stamp) in &old_snapshot.markdown_files {
        match new_snapshot.markdown_files.get(path) {
            Some(new_stamp) if new_stamp == old_stamp => {}
            Some(_) => {
                update.written_paths.insert(path.clone());
            }
            None => {
                update.deleted_paths.insert(path.clone());
            }
        }
    }

    for path in new_snapshot.markdown_files.keys() {
        if !old_snapshot.markdown_files.contains_key(path) {
            update.written_paths.insert(path.clone());
        }
    }

    update
}

fn prepare_full_refresh(
    root: &Path,
    may_need_recovery: bool,
) -> Result<PreparedFullRefresh, CoreError> {
    println!(
        "[uniseq-backend] whole-cache refresh: preparing full refresh at {} (recovery={})",
        root.display(),
        may_need_recovery
    );
    let mut cache = WorkspaceCache::new();
    let recovery_applied = if may_need_recovery || transaction_record_exists(root) {
        recover_workspace_transactions(root, &mut cache)?
    } else {
        false
    };

    if !recovery_applied {
        cache = load_workspace_cache(root)?;
    }

    Ok(PreparedFullRefresh {
        fs_snapshot: WorkspaceFsSnapshot::capture(root)?,
        cache,
        recovery_applied,
    })
}

fn prepare_incremental_fs_update(
    root: &Path,
    update: &IncrementalFsUpdate,
) -> Result<PreparedIncrementalFsUpdate, CoreError> {
    let deleted_paths = update.deleted_paths.iter().cloned().collect::<Vec<_>>();
    let deleted_page_ids = update
        .deleted_paths
        .iter()
        .map(|relative_path| PageId::from_workspace_path(relative_path))
        .collect::<Result<Vec<_>, _>>()?;
    let written_files = update
        .written_paths
        .iter()
        .map(|relative_path| {
            let absolute_path = root.join(relative_path);
            let file_stamp = FileStamp::from_absolute_path(&absolute_path)?;
            let (page, _) = load_page_with_fingerprint_from_relative_path(root, relative_path)?;
            Ok::<PreparedWrittenFile, CoreError>(PreparedWrittenFile {
                relative_path: relative_path.clone(),
                page,
                file_stamp,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PreparedIncrementalFsUpdate {
        deleted_paths,
        deleted_page_ids,
        written_files,
    })
}

fn classify_native_event_burst(root: &Path, events: &[Event]) -> NativeEventAction {
    if events.is_empty() {
        return NativeEventAction::Noop;
    }

    let mut markdown_paths = BTreeSet::new();
    let mut saw_non_markdown_noise = false;

    for event in events {
        if event.paths.is_empty() {
            return NativeEventAction::FallbackToSnapshot;
        }

        let mut event_markdown_path_count = 0usize;
        for path in &event.paths {
            let Ok(relative_path) = path.strip_prefix(root) else {
                return NativeEventAction::FallbackToSnapshot;
            };

            if is_transaction_relative_path(relative_path) {
                return NativeEventAction::TransactionPathChanged;
            }

            let relative_path = relative_path.to_path_buf();
            let is_markdown = relative_path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
            if !is_markdown {
                continue;
            }

            match supported_workspace_markdown_path(&relative_path) {
                Ok(Some(_)) => {
                    event_markdown_path_count += 1;
                    markdown_paths.insert(relative_path);
                }
                Ok(None) => {}
                Err(_) => return NativeEventAction::FallbackToSnapshot,
            }
        }

        if event_markdown_path_count == 0 && !matches!(event.kind, EventKind::Access(_)) {
            saw_non_markdown_noise = true;
        }

    }

    match markdown_paths.len() {
        0 if saw_non_markdown_noise => NativeEventAction::FallbackToSnapshot,
        0 => NativeEventAction::Noop,
        _ if saw_non_markdown_noise => NativeEventAction::FallbackToSnapshot,
        _ => NativeEventAction::IncrementalPaths(markdown_paths),
    }
}

fn collect_native_event_burst(
    first_event: Event,
    rx: &Receiver<WatchLoopMessage>,
) -> Result<Vec<Event>, WatcherFallbackReason> {
    let mut events = vec![first_event];
    while let Ok(message) = rx.try_recv() {
        match message {
            WatchLoopMessage::Fs(Ok(event)) => events.push(event),
            WatchLoopMessage::Fs(Err(error)) => {
                return Err(WatcherFallbackReason::NativeWatcherRuntimeFailed {
                    message: error.to_string(),
                });
            }
            WatchLoopMessage::Stop => return Ok(events),
        }
    }

    Ok(events)
}

fn run_native_or_polling_watch_loop(
    state: Arc<RwLock<WorkspaceSessionState>>,
    rx: Receiver<WatchLoopMessage>,
    tx: Sender<WatchLoopMessage>,
    poll_interval: Duration,
) {
    if let Err(reason) = run_native_watch_loop(&state, &rx, tx, poll_interval) {
        {
            let mut state = state.write().unwrap();
            state.record_watcher_degraded_to_polling(reason);
        }
        run_polling_watch_loop(&state, &rx, poll_interval);
    }
}

fn run_native_watch_loop(
    state: &Arc<RwLock<WorkspaceSessionState>>,
    rx: &Receiver<WatchLoopMessage>,
    tx: Sender<WatchLoopMessage>,
    poll_interval: Duration,
) -> Result<(), WatcherFallbackReason> {
    let root = state.read().unwrap().root.clone();
    let mut watcher = notify::recommended_watcher(move |result| {
        let _ = tx.send(WatchLoopMessage::Fs(result));
    })
    .map_err(|error| WatcherFallbackReason::NativeWatcherSetupFailed {
        message: error.to_string(),
    })?;

    watcher
        .configure(NotifyConfig::default())
        .map_err(|error| WatcherFallbackReason::NativeWatcherSetupFailed {
            message: error.to_string(),
        })?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| WatcherFallbackReason::NativeWatcherSetupFailed {
            message: error.to_string(),
        })?;

    {
        let mut state = state.write().unwrap();
        state.record_watcher_mode(WatcherMode::Native);
        state.watcher_fallback_reason = None;
    }

    loop {
        match rx.recv_timeout(poll_interval) {
            Ok(WatchLoopMessage::Fs(Ok(event))) => {
                let events = collect_native_event_burst(event, rx)?;
                apply_native_event_burst_once(state, &events);
            }
            Ok(WatchLoopMessage::Fs(Err(error))) => {
                return Err(WatcherFallbackReason::NativeWatcherRuntimeFailed {
                    message: error.to_string(),
                });
            }
            Ok(WatchLoopMessage::Stop) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(WatcherFallbackReason::ControlChannelDisconnected);
            }
        }
    }
}

fn run_polling_watch_loop(
    state: &Arc<RwLock<WorkspaceSessionState>>,
    rx: &Receiver<WatchLoopMessage>,
    poll_interval: Duration,
) {
    {
        let mut state = state.write().unwrap();
        state.record_watcher_mode(WatcherMode::Polling);
    }

    loop {
        match rx.recv_timeout(poll_interval) {
            Ok(WatchLoopMessage::Stop) => break,
            Ok(WatchLoopMessage::Fs(_)) => {}
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                poll_state_once(state);
            }
        }
    }
}

fn poll_state_once(state: &Arc<RwLock<WorkspaceSessionState>>) {
    let mut state = state.write().unwrap();
    if let Err(error) = state.poll_once() {
        state.last_watch_error = Some(error);
    }
}

fn apply_native_event_burst_once(state: &Arc<RwLock<WorkspaceSessionState>>, events: &[Event]) {
    let mut state = state.write().unwrap();
    if let Err(error) = state.apply_native_event_burst(events) {
        state.last_watch_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileFingerprint, PageName,
        core::files::{TestWorkspace, workspace_test_relative_path},
        core::structure::stage_page_rename_transaction_for_testing,
    };

    #[test]
    fn startup_recovers_interrupted_transactions_before_reads() {
        let workspace = TestWorkspace::new("uniseq-session");
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
        let workspace = TestWorkspace::new("uniseq-session");
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
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn native_event_hint_refreshes_single_changed_page_without_snapshot_rescan() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- [[C]]\n");
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![workspace.root.join(workspace_test_relative_path("A.md"))],
            attrs: Default::default(),
        };
        session
            .state
            .write()
            .unwrap()
            .apply_native_event_burst(&[event])
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
    }

    #[test]
    fn native_event_burst_keeps_single_page_saves_incremental() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- [[C]]\n");
        workspace.write_file("notes.tmp", "editor noise");
        let events = vec![
            Event {
                kind: EventKind::Modify(notify::event::ModifyKind::Any),
                paths: vec![workspace.root.join(workspace_test_relative_path("A.md"))],
                attrs: Default::default(),
            },
            Event {
                kind: EventKind::Modify(notify::event::ModifyKind::Metadata(
                    notify::event::MetadataKind::Any,
                )),
                paths: vec![workspace.root.join(workspace_test_relative_path("A.md"))],
                attrs: Default::default(),
            },
            Event {
                kind: EventKind::Create(notify::event::CreateKind::Any),
                paths: vec![workspace.root.join("notes.tmp")],
                attrs: Default::default(),
            },
        ];
        session
            .state
            .write()
            .unwrap()
            .apply_native_event_burst(&events)
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
    }

    #[test]
    fn poll_once_adds_created_pages_incrementally() {
        let workspace = TestWorkspace::new("uniseq-session");
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
        let workspace = TestWorkspace::new("uniseq-session");
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
                .incoming_refs
                .len(),
            0
        );
    }

    #[test]
    fn native_multi_file_markdown_bursts_stay_incremental() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("B.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- changed\n");
        workspace.write_file("B.md", "- changed\n");
        let events = vec![
            Event {
                kind: EventKind::Modify(notify::event::ModifyKind::Any),
                paths: vec![workspace.root.join(workspace_test_relative_path("A.md"))],
                attrs: Default::default(),
            },
            Event {
                kind: EventKind::Modify(notify::event::ModifyKind::Any),
                paths: vec![workspace.root.join(workspace_test_relative_path("B.md"))],
                attrs: Default::default(),
            },
        ];
        session
            .state
            .write()
            .unwrap()
            .apply_native_event_burst(&events)
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![PageId::new(["A"]).unwrap(), PageId::new(["B"]).unwrap()],
            }]
        );
    }

    #[test]
    fn poll_once_reconciles_multi_file_markdown_bursts_incrementally() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("B.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- changed\n");
        workspace.write_file("B.md", "- changed\n");
        session.poll_once().unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![PageId::new(["A"]).unwrap(), PageId::new(["B"]).unwrap()],
            }]
        );
    }

    #[test]
    fn full_refresh_materializes_missing_parent_pages() {
        let workspace = TestWorkspace::new("uniseq-session");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A___B___C.md", "");
        session.state.write().unwrap().full_refresh(false).unwrap();

        assert!(workspace
            .root
            .join(workspace_test_relative_path("A.md"))
            .exists());
        assert!(workspace
            .root
            .join(workspace_test_relative_path("A___B.md"))
            .exists());
        let events = session.drain_events();
        assert!(events.contains(&WorkspaceEvent::WorkspaceReloaded));
        assert!(events.contains(&WorkspaceEvent::PagesChanged {
            page_ids: vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap(),
                PageId::new(["A", "B", "C"]).unwrap(),
            ],
        }));
    }

    #[test]
    fn poll_once_falls_back_when_incremental_create_needs_parent_materialization() {
        let workspace = TestWorkspace::new("uniseq-session");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A___B___C.md", "- body\n");
        session.poll_once().unwrap();

        assert!(workspace
            .root
            .join(workspace_test_relative_path("A.md"))
            .exists());
        assert!(workspace
            .root
            .join(workspace_test_relative_path("A___B.md"))
            .exists());
        let events = session.drain_events();
        assert!(events.contains(&WorkspaceEvent::WorkspaceReloaded));
        assert!(events.contains(&WorkspaceEvent::PagesChanged {
            page_ids: vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap(),
                PageId::new(["A", "B", "C"]).unwrap(),
            ],
        }));
    }

    #[test]
    fn native_event_hint_falls_back_when_incremental_create_needs_parent_materialization() {
        let workspace = TestWorkspace::new("uniseq-session");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A___B___C.md", "- body\n");
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::Any),
            paths: vec![workspace.root.join(workspace_test_relative_path("A___B___C.md"))],
            attrs: Default::default(),
        };
        session
            .state
            .write()
            .unwrap()
            .apply_native_event_burst(&[event])
            .unwrap();

        assert!(workspace
            .root
            .join(workspace_test_relative_path("A.md"))
            .exists());
        assert!(workspace
            .root
            .join(workspace_test_relative_path("A___B.md"))
            .exists());
        let events = session.drain_events();
        assert!(events.contains(&WorkspaceEvent::WorkspaceReloaded));
        assert!(events.contains(&WorkspaceEvent::PagesChanged {
            page_ids: vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap(),
                PageId::new(["A", "B", "C"]).unwrap(),
            ],
        }));
    }

    #[test]
    fn direct_content_writes_emit_invalidation_events_and_refresh_cache() {
        let workspace = TestWorkspace::new("uniseq-session");
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
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn classify_snapshot_fs_changes_uses_discovery_stamps() {
        let path = workspace_test_relative_path("A.md");
        let old_snapshot = WorkspaceFsSnapshot {
            markdown_files: BTreeMap::from([(
                path.clone(),
                FileStamp {
                    len_bytes: 8,
                    modified_at_nanos: 1,
                },
            )]),
            transaction_exists: false,
        };
        let new_snapshot = WorkspaceFsSnapshot {
            markdown_files: BTreeMap::from([(
                path.clone(),
                FileStamp {
                    len_bytes: 8,
                    modified_at_nanos: 2,
                },
            )]),
            transaction_exists: false,
        };

        let update = classify_snapshot_fs_changes(&old_snapshot, &new_snapshot);

        assert_eq!(update.written_paths, BTreeSet::from([path]));
        assert!(update.deleted_paths.is_empty());
    }

    #[test]
    fn workspace_snapshot_only_tracks_supported_roots() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("streams/journal/2026-05-07.md", "");
        workspace.write_raw_file("Loose.md", "");
        workspace.write_raw_file("archive/Old.md", "");

        let snapshot = WorkspaceFsSnapshot::capture(&workspace.root).unwrap();

        assert!(snapshot
            .markdown_files
            .contains_key(&workspace_test_relative_path("A.md")));
        assert!(snapshot
            .markdown_files
            .contains_key(&PathBuf::from("streams").join("journal").join("2026-05-07.md")));
        assert!(!snapshot.markdown_files.contains_key(&PathBuf::from("Loose.md")));
        assert!(!snapshot
            .markdown_files
            .contains_key(&PathBuf::from("archive").join("Old.md")));
    }

    #[test]
    fn structural_create_and_delete_emit_cache_invalidation_events() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("X.md", "- [[A/B]]\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        session
            .apply_page_create(PageCreate {
                page_id: PageId::new(["A", "B"]).unwrap(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![
                    PageId::new(["A"]).unwrap(),
                    PageId::new(["A", "B"]).unwrap(),
                    PageId::new(["X"]).unwrap(),
                ],
            }]
        );

        session
            .apply_page_delete_subtree(PageDeleteSubtree {
                page_id: PageId::new(["A"]).unwrap(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![
                WorkspaceEvent::PagesChanged {
                    page_ids: vec![PageId::new(["X"]).unwrap()],
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A"]).unwrap(),
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                },
            ]
        );
    }

    #[test]
    fn stream_create_and_delete_emit_cache_invalidation_events() {
        let workspace = TestWorkspace::new("uniseq-session");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        session
            .apply_stream_page_create(StreamPageCreate {
                stream_name: PageName::new("journal").unwrap(),
                date_name: PageName::new("2026-05-07").unwrap(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PagesChanged {
                page_ids: vec![PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026-05-07").unwrap(),
                )
                .unwrap()],
            }]
        );

        session
            .apply_stream_page_delete(StreamPageDelete {
                stream_name: PageName::new("journal").unwrap(),
                date_name: PageName::new("2026-05-07").unwrap(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![WorkspaceEvent::PageRemoved {
                page_id: PageId::stream(
                    PageName::new("journal").unwrap(),
                    PageName::new("2026-05-07").unwrap(),
                )
                .unwrap(),
            }]
        );
    }

    #[test]
    fn structural_rename_updates_snapshot_without_followup_refresh() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("X.md", "- [[A/B]]\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        session
            .apply_page_rename(PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("C").unwrap(),
            })
            .unwrap();
        session.drain_events();

        {
            let state = session.state.read().unwrap();
            assert!(!state
                .fs_snapshot
                .markdown_files
                .contains_key(&workspace_test_relative_path("A___B.md")));
            assert!(state
                .fs_snapshot
                .markdown_files
                .contains_key(&workspace_test_relative_path("A___C.md")));
            assert!(!state.fs_snapshot.transaction_exists);
        }

        session.poll_once().unwrap();
        assert!(session.drain_events().is_empty());
    }

    #[test]
    fn structural_rename_emits_precise_events_without_workspace_reload() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- [[A/B/C]]\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("X.md", "- [[A/B]] and #A/B/C\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        session
            .apply_page_rename(PageRename {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                new_leaf_name: PageName::new("Renamed").unwrap(),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![
                WorkspaceEvent::PagesChanged {
                    page_ids: vec![
                        PageId::new(["A", "Renamed"]).unwrap(),
                        PageId::new(["A", "Renamed", "C"]).unwrap(),
                        PageId::new(["X"]).unwrap(),
                    ],
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A", "B", "C"]).unwrap(),
                },
            ]
        );
    }

    #[test]
    fn structural_move_emits_precise_events_without_workspace_reload() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- [[A/B/C]]\n");
        workspace.write_file("A___B___C.md", "- child\n");
        workspace.write_file("Z.md", "");
        workspace.write_file("X.md", "- [[A/B]] and #A/B/C\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        session
            .apply_page_move(PageMove {
                source_page_id: PageId::new(["A", "B"]).unwrap(),
                destination_parent_page_id: Some(PageId::new(["Z"]).unwrap()),
            })
            .unwrap();

        assert_eq!(
            session.drain_events(),
            vec![
                WorkspaceEvent::PagesChanged {
                    page_ids: vec![
                        PageId::new(["X"]).unwrap(),
                        PageId::new(["Z", "B"]).unwrap(),
                        PageId::new(["Z", "B", "C"]).unwrap(),
                    ],
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A", "B"]).unwrap(),
                },
                WorkspaceEvent::PageRemoved {
                    page_id: PageId::new(["A", "B", "C"]).unwrap(),
                },
            ]
        );
    }

    #[test]
    fn event_queue_coalesces_pages_changed_and_watcher_mode_updates() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        let mut state = session.state.write().unwrap();

        state.enqueue_event(WorkspaceEvent::PagesChanged {
            page_ids: vec![PageId::new(["B"]).unwrap()],
        });
        state.enqueue_event(WorkspaceEvent::PagesChanged {
            page_ids: vec![PageId::new(["A"]).unwrap(), PageId::new(["B"]).unwrap()],
        });
        state.enqueue_event(WorkspaceEvent::WatcherModeChanged {
            mode: WatcherMode::Native,
        });
        state.enqueue_event(WorkspaceEvent::WatcherModeChanged {
            mode: WatcherMode::Polling,
        });

        assert_eq!(
            state.drain_events(),
            vec![
                WorkspaceEvent::PagesChanged {
                    page_ids: vec![PageId::new(["A"]).unwrap(), PageId::new(["B"]).unwrap(),],
                },
                WorkspaceEvent::WatcherModeChanged {
                    mode: WatcherMode::Polling,
                },
            ]
        );
    }

    #[test]
    fn transaction_dir_event_triggers_recovery_and_workspace_reload() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "- body\n");
        workspace.write_file("X.md", "- [[A/B]]\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        stage_page_rename_transaction_for_testing(
            &workspace.root,
            &PageId::new(["A", "B"]).unwrap(),
            &PageName::new("C").unwrap(),
        )
        .unwrap();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![
                workspace
                    .root
                    .join(".uniseq-page-transaction")
                    .join("manifest.tsv"),
            ],
            attrs: Default::default(),
        };
        session
            .state
            .write()
            .unwrap()
            .apply_native_event_burst(&[event])
            .unwrap();

        let events = session.drain_events();
        assert!(events.contains(&WorkspaceEvent::RecoveryApplied));
        assert!(events.contains(&WorkspaceEvent::WorkspaceReloaded));
        assert!(
            session
                .page_summary(&PageId::new(["A", "C"]).unwrap())
                .is_ok()
        );
        assert!(!workspace.root.join(".uniseq-page-transaction").exists());
    }

    #[test]
    fn poll_once_is_quiet_after_direct_write_is_reconciled_once() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "- old\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();

        workspace.write_file("A.md", "- new\n");
        session.poll_once().unwrap();
        session.drain_events();
        session.poll_once().unwrap();

        assert!(session.drain_events().is_empty());
    }

    #[test]
    fn background_watcher_reports_errors_without_panicking() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "");
        let mut session = WorkspaceSession::open(&workspace.root).unwrap();
        session.drain_events();
        session.start_watching(Duration::from_millis(25));
        thread::sleep(Duration::from_millis(50));

        match session.watcher_mode() {
            Some(WatcherMode::Native) => {
                assert!(session.watcher_fallback_reason().is_none());
            }
            Some(WatcherMode::Polling) => {
                assert!(matches!(
                    session.watcher_fallback_reason(),
                    Some(
                        WatcherFallbackReason::NativeWatcherSetupFailed { .. }
                            | WatcherFallbackReason::NativeWatcherRuntimeFailed { .. }
                            | WatcherFallbackReason::ControlChannelDisconnected
                    )
                ));
            }
            None => panic!("watcher mode was not recorded"),
        }

        fs::create_dir_all(workspace.root.join("nested")).unwrap();
        workspace.write_file("nested\\A.md", "");
        thread::sleep(Duration::from_millis(200));
        session.stop_watching();

        assert!(session.take_last_watch_error().is_none());
        session.poll_once().unwrap();
    }

    #[test]
    fn session_queries_use_current_cache_state() {
        let workspace = TestWorkspace::new("uniseq-session");
        workspace.write_file("A.md", "- body\r\n");
        let session = WorkspaceSession::open(&workspace.root).unwrap();

        let block = session.page_blocks(&PageId::new(["A"]).unwrap()).unwrap();
        assert_eq!(block.revision, FileFingerprint::from_text("- body\r\n"));
        assert_eq!(block.blocks[0].content, "body");
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

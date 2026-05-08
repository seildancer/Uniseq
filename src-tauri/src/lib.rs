use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;
use uniseq_backend::{
    BlockSnapshot, BlockSnapshotKind, CoreError, FileFingerprint, IncomingPageRefSnapshot,
    OutgoingPageRefSnapshot, PageBlocksSnapshot, PageId, PageLocation, PageName, PageSummary,
    SourceSpan, WatcherFallbackReason, WatcherMode, WorkspaceEvent, WorkspaceSession,
};

#[derive(Default)]
struct AppState {
    controller: Mutex<WorkspaceController>,
}

#[derive(Default)]
struct WorkspaceController {
    session: Option<WorkspaceSession>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct WorkspaceOpenDto {
    root_path: String,
    watcher_status: WatcherStatusDto,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct WatcherStatusDto {
    mode: Option<WatcherModeDto>,
    fallback_reason: Option<WatcherFallbackReasonDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PageSummaryDto {
    page_id: String,
    location: PageLocationDto,
    workspace_path: String,
    title: String,
    revision: FileFingerprintDto,
    parent_page_id: Option<String>,
    child_page_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PageDetailDto {
    summary: PageSummaryDto,
    incoming_refs: Vec<IncomingPageRefDto>,
    outgoing_refs: Vec<OutgoingPageRefDto>,
    outgoing_ref_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PageBlocksDto {
    page_id: String,
    revision: FileFingerprintDto,
    blocks: Vec<BlockSnapshotDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct BlockSnapshotDto {
    kind: BlockSnapshotKindDto,
    block_span: SourceSpanDto,
    content_span: SourceSpanDto,
    content: String,
    children: Vec<BlockSnapshotDto>,
    outgoing_refs: Vec<OutgoingPageRefDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct IncomingPageRefDto {
    target_page_id: String,
    source_page_id: String,
    source_page_fingerprint: FileFingerprintDto,
    source_block_span: SourceSpanDto,
    ref_span: SourceSpanDto,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct OutgoingPageRefDto {
    target_page_id: String,
    ref_span: SourceSpanDto,
    target_exists: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct FileFingerprintDto {
    len_bytes: usize,
    content_hash: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SourceSpanDto {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PageLocationDto {
    Pages,
    Stream { stream_name: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WatcherModeDto {
    Native,
    Polling,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WatcherFallbackReasonDto {
    NativeWatcherSetupFailed { message: String },
    NativeWatcherRuntimeFailed { message: String },
    ControlChannelDisconnected,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BlockSnapshotKindDto {
    Outliner,
    ExplicitPlaintext,
    ImplicitPlaintext,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WorkspaceEventDto {
    WorkspaceReloaded,
    PagesChanged { page_ids: Vec<String> },
    PageRemoved { page_id: String },
    WatcherModeChanged { mode: WatcherModeDto },
    WatcherDegradedToPolling { reason: WatcherFallbackReasonDto },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ErrorDto {
    code: &'static str,
    message: String,
    path: Option<String>,
}

type CommandResult<T> = Result<T, ErrorDto>;

impl WorkspaceController {
    fn open_workspace(&mut self, root_path: String) -> CommandResult<WorkspaceOpenDto> {
        let mut session =
            WorkspaceSession::open(PathBuf::from(&root_path)).map_err(ErrorDto::from)?;
        session.start_watching_default();
        let watcher_status = WatcherStatusDto::from_session(&session);
        self.session = Some(session);
        Ok(WorkspaceOpenDto {
            root_path,
            watcher_status,
        })
    }

    fn close_workspace(&mut self) -> bool {
        self.session.take().is_some()
    }

    fn all_pages(&self) -> CommandResult<Vec<PageSummaryDto>> {
        Ok(self
            .session()?
            .all_pages()
            .into_iter()
            .map(PageSummaryDto::from)
            .collect())
    }

    fn page_summary(&self, page_id: String) -> CommandResult<PageSummaryDto> {
        let page_id = parse_page_id_input(&page_id)?;
        self.session()?
            .page_summary(&page_id)
            .map(PageSummaryDto::from)
            .map_err(ErrorDto::from)
    }

    fn page_detail(&self, page_id: String) -> CommandResult<PageDetailDto> {
        let page_id = parse_page_id_input(&page_id)?;
        self.session()?
            .page_detail(&page_id)
            .map(PageDetailDto::from)
            .map_err(ErrorDto::from)
    }

    fn page_blocks(&self, page_id: String) -> CommandResult<PageBlocksDto> {
        let page_id = parse_page_id_input(&page_id)?;
        self.session()?
            .page_blocks(&page_id)
            .map(PageBlocksDto::from)
            .map_err(ErrorDto::from)
    }

    fn page_incoming_refs(&self, page_id: String) -> CommandResult<Vec<IncomingPageRefDto>> {
        let page_id = parse_page_id_input(&page_id)?;
        self.session()?
            .page_incoming_refs(&page_id)
            .map(|refs| refs.into_iter().map(IncomingPageRefDto::from).collect())
            .map_err(ErrorDto::from)
    }

    fn page_outgoing_refs(&self, page_id: String) -> CommandResult<Vec<OutgoingPageRefDto>> {
        let page_id = parse_page_id_input(&page_id)?;
        self.session()?
            .page_outgoing_refs(&page_id)
            .map(|refs| refs.into_iter().map(OutgoingPageRefDto::from).collect())
            .map_err(ErrorDto::from)
    }

    fn drain_workspace_events(&self) -> CommandResult<Vec<WorkspaceEventDto>> {
        let session = self.session()?;
        Ok(session
            .drain_events()
            .into_iter()
            .map(WorkspaceEventDto::from)
            .collect())
    }

    fn take_last_watch_error(&self) -> CommandResult<Option<ErrorDto>> {
        Ok(self.session()?.take_last_watch_error().map(ErrorDto::from))
    }

    fn start_watching(&mut self) -> CommandResult<WatcherStatusDto> {
        let session = self.session_mut()?;
        session.start_watching_default();
        Ok(WatcherStatusDto::from_session(session))
    }

    fn stop_watching(&mut self) -> CommandResult<bool> {
        let session = self.session_mut()?;
        session.stop_watching();
        Ok(true)
    }

    fn session(&self) -> CommandResult<&WorkspaceSession> {
        self.session
            .as_ref()
            .ok_or_else(ErrorDto::no_workspace_open)
    }

    fn session_mut(&mut self) -> CommandResult<&mut WorkspaceSession> {
        self.session
            .as_mut()
            .ok_or_else(ErrorDto::no_workspace_open)
    }
}

impl WatcherStatusDto {
    fn from_session(session: &WorkspaceSession) -> Self {
        Self {
            mode: session.watcher_mode().map(WatcherModeDto::from),
            fallback_reason: session
                .watcher_fallback_reason()
                .map(WatcherFallbackReasonDto::from),
        }
    }
}

impl From<PageSummary> for PageSummaryDto {
    fn from(value: PageSummary) -> Self {
        Self {
            page_id: page_id_to_string(&value.page_id),
            location: PageLocationDto::from(&value.location),
            workspace_path: workspace_path_to_string(&value.workspace_path),
            title: value.title,
            revision: FileFingerprintDto::from(value.revision),
            parent_page_id: value.parent_page_id.as_ref().map(page_id_to_string),
            child_page_count: value.child_page_count,
        }
    }
}

impl From<uniseq_backend::PageDetail> for PageDetailDto {
    fn from(value: uniseq_backend::PageDetail) -> Self {
        Self {
            summary: PageSummaryDto::from(value.summary),
            incoming_refs: value
                .incoming_refs
                .into_iter()
                .map(IncomingPageRefDto::from)
                .collect(),
            outgoing_refs: value
                .outgoing_refs
                .into_iter()
                .map(OutgoingPageRefDto::from)
                .collect(),
            outgoing_ref_count: value.outgoing_ref_count,
        }
    }
}

impl From<PageBlocksSnapshot> for PageBlocksDto {
    fn from(value: PageBlocksSnapshot) -> Self {
        Self {
            page_id: page_id_to_string(&value.page_id),
            revision: FileFingerprintDto::from(value.revision),
            blocks: value
                .blocks
                .into_iter()
                .map(BlockSnapshotDto::from)
                .collect(),
        }
    }
}

impl From<BlockSnapshot> for BlockSnapshotDto {
    fn from(value: BlockSnapshot) -> Self {
        Self {
            kind: BlockSnapshotKindDto::from(value.kind),
            block_span: SourceSpanDto::from(value.block_span),
            content_span: SourceSpanDto::from(value.content_span),
            content: value.content,
            children: value
                .children
                .into_iter()
                .map(BlockSnapshotDto::from)
                .collect(),
            outgoing_refs: value
                .outgoing_refs
                .into_iter()
                .map(OutgoingPageRefDto::from)
                .collect(),
        }
    }
}

impl From<IncomingPageRefSnapshot> for IncomingPageRefDto {
    fn from(value: IncomingPageRefSnapshot) -> Self {
        Self {
            target_page_id: page_id_to_string(&value.target_page_id),
            source_page_id: page_id_to_string(&value.source_page_id),
            source_page_fingerprint: FileFingerprintDto::from(value.source_page_fingerprint),
            source_block_span: SourceSpanDto::from(value.source_block_span),
            ref_span: SourceSpanDto::from(value.ref_span),
        }
    }
}

impl From<OutgoingPageRefSnapshot> for OutgoingPageRefDto {
    fn from(value: OutgoingPageRefSnapshot) -> Self {
        Self {
            target_page_id: page_id_to_string(&value.target_page_id),
            ref_span: SourceSpanDto::from(value.ref_span),
            target_exists: value.target_exists,
        }
    }
}

impl From<FileFingerprint> for FileFingerprintDto {
    fn from(value: FileFingerprint) -> Self {
        Self {
            len_bytes: value.len_bytes(),
            content_hash: value.content_hash(),
        }
    }
}

impl From<SourceSpan> for SourceSpanDto {
    fn from(value: SourceSpan) -> Self {
        Self {
            start: value.start(),
            end: value.end(),
        }
    }
}

impl From<&PageLocation> for PageLocationDto {
    fn from(value: &PageLocation) -> Self {
        match value {
            PageLocation::Pages => Self::Pages,
            PageLocation::Stream { stream_name } => Self::Stream {
                stream_name: stream_name.as_str().to_owned(),
            },
        }
    }
}

impl From<WatcherMode> for WatcherModeDto {
    fn from(value: WatcherMode) -> Self {
        match value {
            WatcherMode::Native => Self::Native,
            WatcherMode::Polling => Self::Polling,
        }
    }
}

impl From<WatcherFallbackReason> for WatcherFallbackReasonDto {
    fn from(value: WatcherFallbackReason) -> Self {
        match value {
            WatcherFallbackReason::NativeWatcherSetupFailed { message } => {
                Self::NativeWatcherSetupFailed { message }
            }
            WatcherFallbackReason::NativeWatcherRuntimeFailed { message } => {
                Self::NativeWatcherRuntimeFailed { message }
            }
            WatcherFallbackReason::ControlChannelDisconnected => Self::ControlChannelDisconnected,
        }
    }
}

impl From<BlockSnapshotKind> for BlockSnapshotKindDto {
    fn from(value: BlockSnapshotKind) -> Self {
        match value {
            BlockSnapshotKind::Outliner => Self::Outliner,
            BlockSnapshotKind::ExplicitPlaintext => Self::ExplicitPlaintext,
            BlockSnapshotKind::ImplicitPlaintext => Self::ImplicitPlaintext,
        }
    }
}

impl From<WorkspaceEvent> for WorkspaceEventDto {
    fn from(value: WorkspaceEvent) -> Self {
        match value {
            WorkspaceEvent::WorkspaceReloaded => Self::WorkspaceReloaded,
            WorkspaceEvent::PagesChanged { page_ids } => Self::PagesChanged {
                page_ids: page_ids.iter().map(page_id_to_string).collect(),
            },
            WorkspaceEvent::PageRemoved { page_id } => Self::PageRemoved {
                page_id: page_id_to_string(&page_id),
            },
            WorkspaceEvent::WatcherModeChanged { mode } => Self::WatcherModeChanged {
                mode: WatcherModeDto::from(mode),
            },
            WorkspaceEvent::WatcherDegradedToPolling { reason } => Self::WatcherDegradedToPolling {
                reason: WatcherFallbackReasonDto::from(reason),
            },
        }
    }
}

impl ErrorDto {
    fn no_workspace_open() -> Self {
        Self {
            code: "no_workspace_open",
            message: "no workspace is currently open".to_owned(),
            path: None,
        }
    }

    fn invalid_page_id(input: &str) -> Self {
        Self {
            code: "invalid_page_id",
            message: format!("invalid page id '{input}'"),
            path: None,
        }
    }
}

impl From<CoreError> for ErrorDto {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::InvalidName(error) => Self {
                code: "invalid_name",
                message: error.to_string(),
                path: None,
            },
            CoreError::InvalidPagePath(error) => Self {
                code: "invalid_page_path",
                message: error.to_string(),
                path: None,
            },
            CoreError::InvalidSpan(error) => Self {
                code: "invalid_span",
                message: error.to_string(),
                path: None,
            },
            CoreError::InvalidParse(error) => Self {
                code: "invalid_parse",
                message: error.to_string(),
                path: None,
            },
            CoreError::DuplicatePageIdentity { page_id } => Self {
                code: "duplicate_page_identity",
                message: format!("duplicate page identity detected for '{page_id}'"),
                path: None,
            },
            CoreError::Io { path, kind } => Self {
                code: "io_error",
                message: format!("i/o error: {kind}"),
                path: Some(workspace_path_to_string(&path)),
            },
            CoreError::StructuralConflict { path } => Self {
                code: "structural_conflict",
                message: "structural operation aborted because the source changed on disk"
                    .to_owned(),
                path: Some(workspace_path_to_string(&path)),
            },
            CoreError::MissingPage => Self {
                code: "missing_page",
                message: "page does not exist in cache".to_owned(),
                path: None,
            },
            CoreError::MissingDestinationParent => Self {
                code: "missing_destination_parent",
                message: "destination parent page does not exist".to_owned(),
                path: None,
            },
            CoreError::DestinationPageExists => Self {
                code: "destination_page_exists",
                message: "destination page already exists".to_owned(),
                path: None,
            },
            CoreError::InvalidPageMove => Self {
                code: "invalid_page_move",
                message: "page move would create an invalid hierarchy".to_owned(),
                path: None,
            },
            CoreError::UnsupportedStreamOperation { operation } => Self {
                code: "unsupported_stream_operation",
                message: format!("stream pages do not support the '{operation}' operation"),
                path: None,
            },
            CoreError::CorruptTransaction => Self {
                code: "corrupt_transaction",
                message: "transaction record is missing or invalid".to_owned(),
                path: None,
            },
        }
    }
}

#[tauri::command]
fn open_workspace(
    state: State<'_, AppState>,
    root_path: String,
) -> CommandResult<WorkspaceOpenDto> {
    state.controller.lock().unwrap().open_workspace(root_path)
}

#[tauri::command]
fn close_workspace(state: State<'_, AppState>) -> bool {
    state.controller.lock().unwrap().close_workspace()
}

#[tauri::command]
fn all_pages(state: State<'_, AppState>) -> CommandResult<Vec<PageSummaryDto>> {
    state.controller.lock().unwrap().all_pages()
}

#[tauri::command]
fn page_summary(state: State<'_, AppState>, page_id: String) -> CommandResult<PageSummaryDto> {
    state.controller.lock().unwrap().page_summary(page_id)
}

#[tauri::command]
fn page_detail(state: State<'_, AppState>, page_id: String) -> CommandResult<PageDetailDto> {
    state.controller.lock().unwrap().page_detail(page_id)
}

#[tauri::command]
fn page_blocks(state: State<'_, AppState>, page_id: String) -> CommandResult<PageBlocksDto> {
    state.controller.lock().unwrap().page_blocks(page_id)
}

#[tauri::command]
fn page_incoming_refs(
    state: State<'_, AppState>,
    page_id: String,
) -> CommandResult<Vec<IncomingPageRefDto>> {
    state.controller.lock().unwrap().page_incoming_refs(page_id)
}

#[tauri::command]
fn page_outgoing_refs(
    state: State<'_, AppState>,
    page_id: String,
) -> CommandResult<Vec<OutgoingPageRefDto>> {
    state.controller.lock().unwrap().page_outgoing_refs(page_id)
}

#[tauri::command]
fn drain_workspace_events(state: State<'_, AppState>) -> CommandResult<Vec<WorkspaceEventDto>> {
    state.controller.lock().unwrap().drain_workspace_events()
}

#[tauri::command]
fn take_last_watch_error(state: State<'_, AppState>) -> CommandResult<Option<ErrorDto>> {
    state.controller.lock().unwrap().take_last_watch_error()
}

#[tauri::command]
fn start_watching(state: State<'_, AppState>) -> CommandResult<WatcherStatusDto> {
    state.controller.lock().unwrap().start_watching()
}

#[tauri::command]
fn stop_watching(state: State<'_, AppState>) -> CommandResult<bool> {
    state.controller.lock().unwrap().stop_watching()
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            close_workspace,
            all_pages,
            page_summary,
            page_detail,
            page_blocks,
            page_incoming_refs,
            page_outgoing_refs,
            drain_workspace_events,
            take_last_watch_error,
            start_watching,
            stop_watching
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}

fn parse_page_id_input(input: &str) -> CommandResult<PageId> {
    if let Some(page_path) = input.strip_prefix("pages:") {
        return PageId::new(page_path.split('/')).map_err(|_| ErrorDto::invalid_page_id(input));
    }

    if let Some(stream_path) = input.strip_prefix("streams/") {
        let mut segments = stream_path.split('/');
        let Some(stream_name) = segments.next() else {
            return Err(ErrorDto::invalid_page_id(input));
        };
        let Some(date_name) = segments.next() else {
            return Err(ErrorDto::invalid_page_id(input));
        };
        if segments.next().is_some() {
            return Err(ErrorDto::invalid_page_id(input));
        }

        let stream_name =
            PageName::new(stream_name).map_err(|_| ErrorDto::invalid_page_id(input))?;
        let date_name = PageName::new(date_name).map_err(|_| ErrorDto::invalid_page_id(input))?;
        return PageId::stream(stream_name, date_name)
            .map_err(|_| ErrorDto::invalid_page_id(input));
    }

    PageId::from_str(input).map_err(|_| ErrorDto::invalid_page_id(input))
}

fn page_id_to_string(page_id: &PageId) -> String {
    page_id.canonical_identity_display()
}

fn workspace_path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{unique}"))
}

#[cfg(test)]
fn write_workspace_file(root: &Path, relative_path: &str, contents: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_id_string_round_trips_for_pages_and_streams() {
        let page = PageId::new(["A", "B"]).unwrap();
        let stream = PageId::stream(
            PageName::new("journal").unwrap(),
            PageName::new("2026-05-07").unwrap(),
        )
        .unwrap();

        assert_eq!(
            parse_page_id_input(&page_id_to_string(&page)).unwrap(),
            page
        );
        assert_eq!(
            parse_page_id_input(&page_id_to_string(&stream)).unwrap(),
            stream
        );
        assert!(parse_page_id_input("streams/journal").is_err());
    }

    #[test]
    fn read_commands_fail_cleanly_without_an_open_workspace() {
        let controller = WorkspaceController::default();

        let error = controller.all_pages().unwrap_err();
        assert_eq!(error.code, "no_workspace_open");
    }

    #[test]
    fn open_workspace_reads_pages_and_exposes_watcher_status() {
        let root = unique_temp_dir("uniseq-desktop-open");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/A.md", "- [[B]]\n");
        write_workspace_file(&root, "pages/B.md", "");

        let mut controller = WorkspaceController::default();
        let opened = controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(opened.root_path, root.to_string_lossy());
        let pages = controller.all_pages().unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_id, "pages:A");

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_events_and_watch_errors_are_surfaceable() {
        let root = unique_temp_dir("uniseq-desktop-events");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/A.md", "- [[B]]\n");
        write_workspace_file(&root, "pages/B.md", "");
        write_workspace_file(&root, "pages/C.md", "");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();
        let _ = controller.drain_workspace_events().unwrap();

        write_workspace_file(&root, "pages/A.md", "- [[C]]\n");
        controller.session().unwrap().poll_once().unwrap();

        let events = controller.drain_workspace_events().unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            WorkspaceEventDto::PagesChanged { page_ids }
                if page_ids == &vec![
                    "pages:A".to_owned(),
                    "pages:B".to_owned(),
                    "pages:C".to_owned()
                ]
        )));
        assert!(controller.take_last_watch_error().unwrap().is_none());

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }
}

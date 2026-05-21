use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uniseq_backend::{
    BlockHandle, BlockSnapshot, CoreError, FileFingerprint, FlatBlockSnapshot,
    IncomingPageRefSnapshot, LinkedRefEntry, OutgoingPageRefSnapshot, PageContentSnapshot,
    PageCreate, PageDeleteSubtree, PageId, PageLocation, PageMerge, PageMove, PageName, PageRename,
    PageSummary, RefHighlightSnapshot, SearchMatchField, SearchResult, SourceSpan,
    StreamPageCreate, StreamPageDelete, WatcherFallbackReason, WatcherMode, WorkspaceEvent,
    WorkspaceSession, create_workspace_root, prepare_workspace_root,
};

mod ai;
mod sync;

const LAST_WORKSPACE_FILE_NAME: &str = "last-workspace.txt";
const PAGE_ORDER_FILE_NAME: &str = "page-order.json";
const OLD_PAGE_ORDER_STORE_FILE_NAME: &str = "workspace-page-order.json";
const ROOT_PARENT_ORDER_KEY: &str = "__root__";
const SYNC_PROGRESS_EVENT: &str = "sync-progress";
const DEFAULT_WORKSPACE_FOLDER_NAME: &str = "default_workspace";
const AI_CHAT_CONTEXT_TOKEN_BUDGET: usize = 100_000;
const AI_CHAT_APPROX_CHARS_PER_TOKEN: usize = 4;

#[derive(Default)]
struct AppState {
    controller: Mutex<WorkspaceController>,
    sync_lock: Arc<Mutex<()>>,
}

struct SyncLoop {
    sender: mpsc::Sender<sync::SyncLoopMessage>,
    handle: Option<thread::JoinHandle<()>>,
}

impl SyncLoop {
    fn stop(mut self) {
        let _ = self.sender.send(sync::SyncLoopMessage::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SyncLoop {
    fn drop(&mut self) {
        let _ = self.sender.send(sync::SyncLoopMessage::Stop);
    }
}

#[derive(Default)]
struct WorkspaceController {
    session: Option<WorkspaceSession>,
    sync_loop: Option<SyncLoop>,
    active_ai_chat_session: Option<ActiveAiChatSession>,
    next_ai_chat_session_nonce: u64,
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
    modified_at: Option<u64>,
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
struct FlatBlockDto {
    kind: String,
    depth: u32,
    content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PageContentDto {
    revision: FileFingerprintDto,
    blocks: Vec<FlatBlockDto>,
    text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PageOrderDto {
    sibling_order_by_parent: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct CleanupResultDto {
    removed_page_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DeleteStreamResultDto {
    deleted_page_count: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BlockHandleDto {
    source_page_id: String,
    source_page_revision: FileFingerprintDto,
    block_span: SourceSpanDto,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct BlockSnapshotDto {
    handle: BlockHandleDto,
    kind: String,
    block_span: SourceSpanDto,
    content_span: SourceSpanDto,
    content: String,
    markdown: String,
    outgoing_refs: Vec<OutgoingPageRefDto>,
    children: Vec<BlockSnapshotDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct RefHighlightDto {
    prefix: String,
    highlight: String,
    suffix: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LinkedRefEntryDto {
    target_page_id: String,
    source_page_id: String,
    ref_span: SourceSpanDto,
    block: BlockSnapshotDto,
    block_content_highlight: Option<RefHighlightDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SearchResultDto {
    page_id: String,
    title: String,
    location: PageLocationDto,
    matched_field: String,
    snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileFingerprintDto {
    len_bytes: usize,
    #[serde(with = "u64_string")]
    content_hash: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct FileFingerprintInputDto {
    len_bytes: usize,
    #[serde(with = "u64_string")]
    content_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[serde(tag = "type", rename_all = "snake_case")]
enum WorkspaceEventDto {
    WorkspaceReloaded,
    PagesChanged { page_ids: Vec<String> },
    PageRemoved { page_id: String },
    WatcherModeChanged { mode: WatcherModeDto },
    WatcherDegradedToPolling { reason: WatcherFallbackReasonDto },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SyncProviderKindDto {
    Uniseq,
    Custom,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ErrorDto {
    code: &'static str,
    message: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct PageOrderStore {
    workspaces: BTreeMap<String, WorkspacePageOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct WorkspacePageOrder {
    sibling_order_by_parent: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AiChatContextSpecDto {
    Page { page_id: String },
    StreamSingle { stream_name: String },
    StreamDual { stream_names: Vec<String> },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AiChatSessionOpenDto {
    session_id: String,
    preview_summary: String,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AiChatResponseDto {
    assistant_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AiChatMessageDto {
    role: String,
    content: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
struct PreparedAiChatSession {
    system_prompt: String,
    preview_summary: String,
    truncated: bool,
    included_page_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct ActiveAiChatSession {
    session_id: String,
    system_prompt: String,
}

type CommandResult<T> = Result<T, ErrorDto>;

mod u64_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<u64>().map_err(serde::de::Error::custom)
    }
}

impl WorkspaceController {
    fn open_workspace(&mut self, root_path: String) -> CommandResult<WorkspaceOpenDto> {
        let root_path = PathBuf::from(root_path);
        prepare_workspace_root(&root_path).map_err(ErrorDto::from)?;
        let mut session = WorkspaceSession::open(&root_path).map_err(ErrorDto::from)?;
        session.start_watching_default();
        let watcher_status = WatcherStatusDto::from_session(&session);
        self.session = Some(session);
        self.active_ai_chat_session = None;
        Ok(WorkspaceOpenDto {
            root_path: root_path.to_string_lossy().to_string(),
            watcher_status,
        })
    }

    fn create_workspace(
        &mut self,
        parent_path: String,
        folder_name: String,
    ) -> CommandResult<WorkspaceOpenDto> {
        let root_path =
            create_workspace_root(&parent_path, &folder_name).map_err(ErrorDto::from)?;
        let opened = self.open_workspace(root_path.to_string_lossy().to_string())?;
        self.seed_new_workspace();
        Ok(opened)
    }

    fn seed_new_workspace(&self) {
        let today = today_date_name();
        let _ = self.write_page_content(
            format!("stream:journals/{today}"),
            "A block of text or an outliner block can be tagged to a #page".to_string(),
            None,
        );
        let _ = self.write_page_content(
            format!("stream:diary/{today}"),
            "`[[page]]` is another way to refer to a [[page]]".to_string(),
            None,
        );
        let _ = self.create_page("pages:page".to_string());
        let _ = self.write_page_content(
            "pages:page".to_string(),
            "We suggest timeless, persistent information to reside here.".to_string(),
            None,
        );
        let _ = self.create_page("pages:page/subpage".to_string());
        let _ = self.write_page_content(
            "pages:page/subpage".to_string(),
            "subpages can be made as well!".to_string(),
            None,
        );
    }

    fn close_workspace(&mut self) -> bool {
        self.stop_sync_loop();
        self.active_ai_chat_session = None;
        self.session.take().is_some()
    }

    fn reopen_workspace(&mut self) -> CommandResult<()> {
        let workspace_root = self.session()?.workspace_root();
        self.open_workspace(workspace_root.to_string_lossy().to_string())
            .map(|_| ())
    }

    fn open_ai_chat_session(
        &mut self,
        context_spec: AiChatContextSpecDto,
    ) -> CommandResult<AiChatSessionOpenDto> {
        let prepared = build_ai_chat_session(
            self.session()?,
            &context_spec,
            ai_context_char_budget(),
        )?;
        self.next_ai_chat_session_nonce = self.next_ai_chat_session_nonce.saturating_add(1);
        let session_id = format!("ai-session-{}", self.next_ai_chat_session_nonce);
        self.active_ai_chat_session = Some(ActiveAiChatSession {
            session_id: session_id.clone(),
            system_prompt: prepared.system_prompt,
        });
        Ok(AiChatSessionOpenDto {
            session_id,
            preview_summary: prepared.preview_summary,
            truncated: prepared.truncated,
        })
    }

    fn close_ai_chat_session(&mut self, session_id: String) -> bool {
        if self
            .active_ai_chat_session
            .as_ref()
            .is_some_and(|session| session.session_id == session_id)
        {
            self.active_ai_chat_session = None;
            return true;
        }
        false
    }

    fn all_pages(&self) -> CommandResult<Vec<PageSummaryDto>> {
        Ok(self
            .session()?
            .all_pages()
            .into_iter()
            .map(PageSummaryDto::from)
            .collect())
    }

    fn find_empty_pages(&self) -> CommandResult<Vec<PageSummaryDto>> {
        let mut empty_pages = Vec::new();

        for page in self.session()?.all_pages() {
            if matches!(page.location, PageLocation::Pages) && page.child_page_count > 0 {
                continue;
            }

            if !self
                .session()?
                .page_linked_refs(&page.page_id)
                .map_err(ErrorDto::from)?
                .is_empty()
            {
                continue;
            }

            let content = self
                .session()?
                .page_content(&page.page_id)
                .map_err(ErrorDto::from)?;
            if content.text.trim().is_empty() {
                empty_pages.push(PageSummaryDto::from(page));
            }
        }

        Ok(empty_pages)
    }

    fn all_streams(&self) -> CommandResult<Vec<String>> {
        self.session()?.all_streams().map_err(ErrorDto::from)
    }

    fn page_order(&self, app: &AppHandle) -> CommandResult<PageOrderDto> {
        let workspace_root = self.session()?.workspace_root();
        let pages = self.session()?.all_pages();
        let should_materialize_workspace_order =
            !workspace_page_order_path(&workspace_root).exists();
        let workspace_order = read_workspace_page_order(app, &workspace_root)?;
        let normalized = normalize_workspace_page_order(&workspace_order, &pages);
        if should_materialize_workspace_order || normalized != workspace_order {
            write_workspace_page_order(&workspace_root, &normalized)?;
        }
        Ok(PageOrderDto {
            sibling_order_by_parent: normalized.sibling_order_by_parent,
        })
    }

    fn page_summary(&self, page_id: String) -> CommandResult<PageSummaryDto> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_summary(&page_id)
            .map(PageSummaryDto::from)
            .map_err(ErrorDto::from)
    }

    fn page_detail(&self, page_id: String) -> CommandResult<PageDetailDto> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_detail(&page_id)
            .map(PageDetailDto::from)
            .map_err(ErrorDto::from)
    }

    fn page_content(&self, page_id: String) -> CommandResult<PageContentDto> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_content(&page_id)
            .map(PageContentDto::from)
            .map_err(ErrorDto::from)
    }

    fn search_pages(
        &self,
        page_query: String,
        limit: Option<usize>,
    ) -> CommandResult<Vec<SearchResultDto>> {
        let limit = limit.unwrap_or(50);
        Ok(self
            .session()?
            .search_pages(&page_query, limit)
            .into_iter()
            .map(SearchResultDto::from)
            .collect())
    }

    fn write_page_content(
        &self,
        page_id: String,
        text: String,
        expected_revision: Option<FileFingerprintInputDto>,
    ) -> CommandResult<PageContentDto> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        let expected_revision = expected_revision.map(FileFingerprint::from);
        let session = self.session()?;
        let result = match session.write_and_reparse(&page_id, text.clone(), expected_revision) {
            Ok(snapshot) => Ok(snapshot),
            Err(CoreError::MissingPage)
                if expected_revision.is_none()
                    && matches!(page_id.location(), PageLocation::Stream { .. }) =>
            {
                let PageLocation::Stream { stream_name } = page_id.location().clone() else {
                    unreachable!("stream-backed page ids must expose stream locations");
                };
                session
                    .apply_stream_page_create(StreamPageCreate {
                        stream_name,
                        date_name: page_id.leaf_name().clone(),
                    })
                    .map_err(ErrorDto::from)?;
                session.write_and_reparse(&page_id, text, None)
            }
            Err(error) => Err(error),
        }
        .map(PageContentDto::from)
        .map_err(ErrorDto::from);
        self.finish_local_mutation(result)
    }

    fn create_stream_page(
        &self,
        stream_name: String,
        date_name: String,
    ) -> CommandResult<PageSummaryDto> {
        let stream_name = PageName::new(&stream_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        let date_name = PageName::new(&date_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        let page_id = PageId::stream(stream_name.clone(), date_name.clone())
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        self.session()?
            .apply_stream_page_create(StreamPageCreate {
                stream_name,
                date_name,
            })
            .map_err(ErrorDto::from)?;
        let result = self
            .session()?
            .page_summary(&page_id)
            .map(PageSummaryDto::from)
            .map_err(ErrorDto::from);
        self.finish_local_mutation(result)
    }

    fn delete_stream_page(&self, stream_name: String, date_name: String) -> CommandResult<()> {
        let stream_name = PageName::new(&stream_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        let date_name = PageName::new(&date_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        let result = self
            .session()?
            .apply_stream_page_delete(StreamPageDelete {
                stream_name,
                date_name,
            })
            .map_err(ErrorDto::from);
        self.finish_local_mutation(result)
    }

    fn delete_stream(&self, stream_name: String) -> CommandResult<DeleteStreamResultDto> {
        let stream_name_parsed = PageName::new(&stream_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;

        let all_pages = self.session()?.all_pages();
        let mut deleted_count = 0;

        for page in &all_pages {
            let page_stream_name = match &page.location {
                PageLocation::Stream { stream_name: sn } => sn,
                PageLocation::Pages => continue,
            };
            if page_stream_name.as_str() != stream_name_parsed.as_str() {
                continue;
            }
            let date_name_str = page.page_id.leaf_name().as_str().to_owned();
            if let Ok(date_name) = PageName::new(&date_name_str) {
                if self
                    .session()?
                    .apply_stream_page_delete(StreamPageDelete {
                        stream_name: stream_name_parsed.clone(),
                        date_name,
                    })
                    .is_ok()
                {
                    deleted_count += 1;
                }
            }
        }

        let workspace_root = self.session()?.workspace_root();
        let stream_dir = workspace_root.join(stream_name_parsed.as_str());
        let _ = fs::remove_dir(&stream_dir);

        let result = Ok(DeleteStreamResultDto {
            deleted_page_count: deleted_count,
        });
        self.finish_local_mutation(result)
    }

    fn rename_stream(&mut self, stream_name: String, new_stream_name: String) -> CommandResult<()> {
        let stream_name = PageName::new(&stream_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        let new_stream_name = PageName::new(&new_stream_name)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;

        if stream_name == new_stream_name {
            return Ok(());
        }

        let workspace_root = self.session()?.workspace_root();
        let source_dir = workspace_root.join(stream_name.as_str());
        let target_dir = workspace_root.join(new_stream_name.as_str());

        if target_dir.exists() {
            return Err(ErrorDto::from(CoreError::DestinationPageExists));
        }

        fs::rename(&source_dir, &target_dir)
            .map_err(|error| ErrorDto::from(CoreError::io(&source_dir, &error)))?;
        let result = self.reopen_workspace();
        self.finish_local_mutation(result)
    }

    fn cleanup_empty_stream_pages(&self, older_than_days: u64) -> CommandResult<CleanupResultDto> {
        let all_pages = self.session()?.all_pages();
        let mut removed_page_ids = Vec::new();

        for page_summary in all_pages {
            let stream_name = match &page_summary.location {
                PageLocation::Stream { stream_name } => stream_name.clone(),
                PageLocation::Pages => continue,
            };

            let date_name_str = page_summary.page_id.leaf_name().as_str().to_owned();
            let age_days = match days_since_date_name(&date_name_str) {
                Some(age) => age,
                None => continue,
            };
            if age_days < older_than_days {
                continue;
            }

            let content = match self.session()?.page_content(&page_summary.page_id) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if !content.text.trim().is_empty() {
                continue;
            }

            let date_name = match PageName::new(&date_name_str) {
                Ok(n) => n,
                Err(_) => continue,
            };
            if self
                .session()?
                .apply_stream_page_delete(StreamPageDelete {
                    stream_name,
                    date_name,
                })
                .is_ok()
            {
                removed_page_ids.push(page_id_to_string(&page_summary.page_id));
            }
        }

        let result = Ok(CleanupResultDto { removed_page_ids });
        self.finish_local_mutation(result)
    }

    fn refresh_stream_workspace(&self, older_than_days: u64) -> CommandResult<CleanupResultDto> {
        self.cleanup_empty_stream_pages(older_than_days)
    }

    fn create_page(&self, page_id: String) -> CommandResult<()> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        let result = self
            .session()?
            .apply_page_create(PageCreate { page_id })
            .map_err(ErrorDto::from);
        self.finish_local_mutation(result)
    }

    fn rename_page(
        &self,
        app: &AppHandle,
        page_id: String,
        new_title: String,
    ) -> CommandResult<()> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        let new_leaf_name = PageName::new(&new_title).map_err(CoreError::from)?;
        let target_page_id = renamed_page_id_string(&page_id, &new_leaf_name);
        self.session()?
            .apply_page_rename(PageRename {
                source_page_id: page_id.clone(),
                new_leaf_name,
            })
            .map_err(ErrorDto::from)?;
        self.finish_local_mutation(self.remap_page_order_subtree(
            app,
            &page_id_to_string(&page_id),
            &target_page_id,
        ))
    }

    fn move_page(
        &self,
        app: &AppHandle,
        page_id: String,
        new_parent_page_id: Option<String>,
    ) -> CommandResult<()> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        let destination_parent_page_id = match new_parent_page_id {
            Some(id) => Some(parse_page_id_input(&id).map_err(|_| ErrorDto::invalid_page_id(&id))?),
            None => None,
        };
        let target_page_id = moved_page_id_string(&page_id, destination_parent_page_id.as_ref());
        self.session()?
            .apply_page_move(PageMove {
                source_page_id: page_id.clone(),
                destination_parent_page_id,
            })
            .map_err(ErrorDto::from)?;
        self.finish_local_mutation(self.remap_page_order_subtree(
            app,
            &page_id_to_string(&page_id),
            &target_page_id,
        ))
    }

    fn delete_page(&self, app: &AppHandle, page_id: String) -> CommandResult<()> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .apply_page_delete_subtree(PageDeleteSubtree {
                page_id: page_id.clone(),
            })
            .map_err(ErrorDto::from)?;
        self.finish_local_mutation(
            self.remove_page_order_subtree(app, &page_id_to_string(&page_id)),
        )
    }

    fn merge_page(
        &self,
        app: &AppHandle,
        source_page_id: String,
        target_page_id: String,
    ) -> CommandResult<()> {
        let source_page_id = parse_page_id_input(&source_page_id)
            .map_err(|_| ErrorDto::invalid_page_id(&source_page_id))?;
        let target_page_id = parse_page_id_input(&target_page_id)
            .map_err(|_| ErrorDto::invalid_page_id(&target_page_id))?;
        self.session()?
            .apply_page_merge(PageMerge {
                source_page_id: source_page_id.clone(),
                target_page_id,
            })
            .map_err(ErrorDto::from)?;
        self.finish_local_mutation(
            self.remove_page_order_subtree(app, &page_id_to_string(&source_page_id)),
        )
    }

    fn set_page_sibling_order(
        &self,
        app: &AppHandle,
        parent_page_id: Option<String>,
        ordered_child_page_ids: Vec<String>,
    ) -> CommandResult<()> {
        let canonical_parent_page_id = parent_page_id
            .as_deref()
            .map(parse_page_id_input)
            .transpose()
            .map_err(|_| {
                parent_page_id
                    .as_deref()
                    .map(ErrorDto::invalid_page_id)
                    .unwrap_or_else(ErrorDto::no_workspace_open)
            })?
            .as_ref()
            .map(page_id_to_string);
        let ordered_child_page_ids = ordered_child_page_ids
            .iter()
            .map(|page_id| {
                parse_page_id_input(page_id)
                    .map(|parsed| page_id_to_string(&parsed))
                    .map_err(|_| ErrorDto::invalid_page_id(page_id))
            })
            .collect::<CommandResult<Vec<_>>>()?;

        let workspace_root = self.session()?.workspace_root();
        let pages = self.session()?.all_pages();
        let mut workspace_order = read_workspace_page_order(app, &workspace_root)?;
        workspace_order.sibling_order_by_parent.insert(
            parent_order_key(canonical_parent_page_id.as_deref()),
            ordered_child_page_ids,
        );
        workspace_order = normalize_workspace_page_order(&workspace_order, &pages);
        let result = write_workspace_page_order(&workspace_root, &workspace_order);
        self.finish_local_mutation(result)
    }

    fn page_incoming_refs(&self, page_id: String) -> CommandResult<Vec<IncomingPageRefDto>> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_incoming_refs(&page_id)
            .map(|refs| refs.into_iter().map(IncomingPageRefDto::from).collect())
            .map_err(ErrorDto::from)
    }

    fn page_outgoing_refs(&self, page_id: String) -> CommandResult<Vec<OutgoingPageRefDto>> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_outgoing_refs(&page_id)
            .map(|refs| refs.into_iter().map(OutgoingPageRefDto::from).collect())
            .map_err(ErrorDto::from)
    }

    fn page_linked_refs(&self, page_id: String) -> CommandResult<Vec<LinkedRefEntryDto>> {
        let page_id =
            parse_page_id_input(&page_id).map_err(|_| ErrorDto::invalid_page_id(&page_id))?;
        self.session()?
            .page_linked_refs(&page_id)
            .map(|refs| refs.into_iter().map(LinkedRefEntryDto::from).collect())
            .map_err(ErrorDto::from)
    }

    fn block_snapshot(&self, handle: BlockHandleDto) -> CommandResult<BlockSnapshotDto> {
        let handle = BlockHandle::try_from(handle)?;
        self.session()?
            .block_snapshot(&handle)
            .map(BlockSnapshotDto::from)
            .map_err(ErrorDto::from)
    }

    fn write_block_markdown(
        &self,
        handle: BlockHandleDto,
        replacement_markdown: String,
    ) -> CommandResult<BlockSnapshotDto> {
        let handle = BlockHandle::try_from(handle)?;
        let result = self
            .session()?
            .write_block_markdown(&handle, replacement_markdown)
            .map(BlockSnapshotDto::from)
            .map_err(ErrorDto::from);
        self.finish_local_mutation(result)
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

    fn configure_sync(
        &self,
        provider: SyncProviderKindDto,
        sync_root_url: String,
        remote_workspace_id: String,
        remote_workspace_name: String,
        auth_kind: Option<sync::SyncAuthKind>,
        auth_token: Option<String>,
        refresh_token: Option<String>,
        supabase_publishable_key: Option<String>,
    ) -> CommandResult<sync::SyncStatus> {
        let workspace_root = self.session()?.workspace_root();
        let config = sync::SyncConfig::new_with_auth(
            sync::SyncProviderKind::from(provider),
            sync_root_url,
            remote_workspace_id,
            remote_workspace_name,
            sync::SyncAuthConfig {
                kind: auth_kind.unwrap_or_default(),
            },
        );
        sync::write_sync_config(&workspace_root, &config).map_err(ErrorDto::from)?;
        sync::write_sync_auth_secrets(
            &workspace_root,
            &sync::SyncAuthSecrets {
                bearer_token: normalize_auth_token(auth_token),
                refresh_token: normalize_auth_token(refresh_token),
                supabase_publishable_key: normalize_auth_token(supabase_publishable_key),
            },
        )
        .map_err(ErrorDto::from)?;
        self.sync_status_for_workspace(&workspace_root)
    }

    fn set_sync_enabled(&self, enabled: bool) -> CommandResult<sync::SyncStatus> {
        let workspace_root = self.session()?.workspace_root();
        let mut config = sync::read_sync_config(&workspace_root)
            .map_err(ErrorDto::from)?
            .ok_or_else(|| ErrorDto::sync("sync is not configured"))?;
        config.enabled = enabled;
        sync::write_sync_config(&workspace_root, &config).map_err(ErrorDto::from)?;
        self.sync_status_for_workspace(&workspace_root)
    }

    fn sync_status(&self) -> CommandResult<sync::SyncStatus> {
        let workspace_root = self.session()?.workspace_root();
        self.sync_status_for_workspace(&workspace_root)
    }

    fn sync_conflict_detail(&self, path: String) -> CommandResult<sync::SyncConflictDetail> {
        let workspace_root = self.session()?.workspace_root();
        let config = sync::read_sync_config(&workspace_root)
            .map_err(ErrorDto::from)?
            .ok_or_else(|| ErrorDto::sync("sync is not configured"))?;
        let provider = sync_provider_for_config(&workspace_root, &config)?;
        sync::conflict_detail(&workspace_root, &provider, &path).map_err(ErrorDto::from)
    }

    fn resolve_sync_conflict(
        &mut self,
        path: String,
        resolution: sync::SyncConflictResolution,
    ) -> CommandResult<sync::SyncRunSummary> {
        let workspace_root = self.session()?.workspace_root();
        let config = sync::read_sync_config(&workspace_root)
            .map_err(ErrorDto::from)?
            .ok_or_else(|| ErrorDto::sync("sync is not configured"))?;
        let provider = sync_provider_for_config(&workspace_root, &config)?;
        let summary = sync::resolve_conflict(&workspace_root, &provider, &path, resolution)
            .map_err(ErrorDto::from)?;
        if summary.pulled > 0 {
            self.reopen_workspace()?;
        }
        Ok(self.decorate_sync_summary(summary))
    }

    fn start_sync_loop(&mut self, app: AppHandle, sync_lock: Arc<Mutex<()>>) {
        self.stop_sync_loop();
        let Some(root) = self.session.as_ref().map(|s| s.workspace_root()) else {
            return;
        };
        let config_enabled = matches!(
            sync::read_sync_config(&root),
            Ok(Some(ref c)) if c.enabled
        );
        if !config_enabled {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || sync::run_sync_loop(root, app, sync_lock, rx));
        self.sync_loop = Some(SyncLoop {
            sender: tx,
            handle: Some(handle),
        });
    }

    fn stop_sync_loop(&mut self) {
        if let Some(loop_) = self.sync_loop.take() {
            loop_.stop();
        }
    }

    fn notify_local_write(&self) {
        if let Some(loop_) = &self.sync_loop {
            let _ = loop_.sender.send(sync::SyncLoopMessage::LocalWrite);
        }
    }

    fn notify_user_activity(&self) {
        if let Some(loop_) = &self.sync_loop {
            let _ = loop_.sender.send(sync::SyncLoopMessage::UserActivity);
        }
    }

    fn finish_local_mutation<T>(&self, result: CommandResult<T>) -> CommandResult<T> {
        if result.is_ok() {
            self.notify_local_write();
        }
        result
    }

    fn sync_loop_running(&self) -> bool {
        self.sync_loop.is_some()
    }

    fn decorate_sync_status(&self, mut status: sync::SyncStatus) -> sync::SyncStatus {
        status.background_loop_running = self.sync_loop_running();
        status
    }

    fn decorate_sync_summary(&self, mut summary: sync::SyncRunSummary) -> sync::SyncRunSummary {
        summary.status = self.decorate_sync_status(summary.status);
        summary
    }

    fn sync_status_for_workspace(&self, workspace_root: &Path) -> CommandResult<sync::SyncStatus> {
        sync::sync_status(workspace_root)
            .map(|status| self.decorate_sync_status(status))
            .map_err(ErrorDto::from)
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

    fn active_ai_chat_session(
        &self,
        session_id: &str,
    ) -> CommandResult<&ActiveAiChatSession> {
        self.active_ai_chat_session
            .as_ref()
            .filter(|session| session.session_id == session_id)
            .ok_or_else(ErrorDto::ai_chat_session_missing)
    }

    fn remap_page_order_subtree(
        &self,
        app: &AppHandle,
        source_page_id: &str,
        target_page_id: &str,
    ) -> CommandResult<()> {
        let workspace_root = self.session()?.workspace_root();
        let pages = self.session()?.all_pages();
        let mut workspace_order = read_workspace_page_order(app, &workspace_root)?;
        remap_workspace_page_order_subtree(&mut workspace_order, source_page_id, target_page_id);
        workspace_order = normalize_workspace_page_order(&workspace_order, &pages);
        write_workspace_page_order(&workspace_root, &workspace_order)
    }

    fn remove_page_order_subtree(
        &self,
        app: &AppHandle,
        source_page_id: &str,
    ) -> CommandResult<()> {
        let workspace_root = self.session()?.workspace_root();
        let pages = self.session()?.all_pages();
        let mut workspace_order = read_workspace_page_order(app, &workspace_root)?;
        remove_workspace_page_order_subtree(&mut workspace_order, source_page_id);
        workspace_order = normalize_workspace_page_order(&workspace_order, &pages);
        write_workspace_page_order(&workspace_root, &workspace_order)
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
            modified_at: value.modified_at,
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

impl From<PageContentSnapshot> for PageContentDto {
    fn from(value: PageContentSnapshot) -> Self {
        Self {
            revision: FileFingerprintDto::from(value.revision),
            blocks: value.blocks.into_iter().map(FlatBlockDto::from).collect(),
            text: value.text,
        }
    }
}

impl From<FlatBlockSnapshot> for FlatBlockDto {
    fn from(value: FlatBlockSnapshot) -> Self {
        Self {
            kind: match value.kind {
                uniseq_backend::BlockKind::Outliner => "outliner".to_owned(),
                uniseq_backend::BlockKind::Plaintext => "plaintext".to_owned(),
            },
            depth: value.depth,
            content: value.content,
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

impl From<BlockSnapshot> for BlockSnapshotDto {
    fn from(value: BlockSnapshot) -> Self {
        Self {
            handle: BlockHandleDto::from(value.handle),
            kind: match value.kind {
                uniseq_backend::BlockKind::Outliner => "outliner".to_owned(),
                uniseq_backend::BlockKind::Plaintext => "plaintext".to_owned(),
            },
            block_span: SourceSpanDto::from(value.block_span),
            content_span: SourceSpanDto::from(value.content_span),
            content: value.content,
            markdown: value.markdown,
            outgoing_refs: value
                .outgoing_refs
                .into_iter()
                .map(OutgoingPageRefDto::from)
                .collect(),
            children: value
                .children
                .into_iter()
                .map(BlockSnapshotDto::from)
                .collect(),
        }
    }
}

impl From<BlockHandle> for BlockHandleDto {
    fn from(value: BlockHandle) -> Self {
        Self {
            source_page_id: page_id_to_string(&value.source_page_id),
            source_page_revision: FileFingerprintDto::from(value.source_page_revision),
            block_span: SourceSpanDto::from(value.block_span),
        }
    }
}

impl TryFrom<BlockHandleDto> for BlockHandle {
    type Error = ErrorDto;

    fn try_from(value: BlockHandleDto) -> Result<Self, Self::Error> {
        let source_page_id = parse_page_id_input(&value.source_page_id)
            .map_err(|_| ErrorDto::invalid_page_id(&value.source_page_id))?;
        let block_span = SourceSpan::new(value.block_span.start, value.block_span.end)
            .map_err(CoreError::from)
            .map_err(ErrorDto::from)?;
        Ok(Self {
            source_page_id,
            source_page_revision: FileFingerprint::from(value.source_page_revision),
            block_span,
        })
    }
}

impl From<RefHighlightSnapshot> for RefHighlightDto {
    fn from(value: RefHighlightSnapshot) -> Self {
        Self {
            prefix: value.prefix,
            highlight: value.highlight,
            suffix: value.suffix,
        }
    }
}

impl From<LinkedRefEntry> for LinkedRefEntryDto {
    fn from(value: LinkedRefEntry) -> Self {
        Self {
            target_page_id: page_id_to_string(&value.target_page_id),
            source_page_id: page_id_to_string(&value.source_page_id),
            ref_span: SourceSpanDto::from(value.ref_span),
            block: BlockSnapshotDto::from(value.block),
            block_content_highlight: value.block_content_highlight.map(RefHighlightDto::from),
        }
    }
}

impl From<SearchResult> for SearchResultDto {
    fn from(value: SearchResult) -> Self {
        Self {
            page_id: page_id_to_string(&value.page_id),
            title: value.title,
            location: PageLocationDto::from(&value.location),
            matched_field: match value.matched_field {
                SearchMatchField::Title => "title".to_owned(),
                SearchMatchField::PageId => "page_id".to_owned(),
                SearchMatchField::Content => "content".to_owned(),
            },
            snippet: value.snippet,
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

impl From<FileFingerprintInputDto> for FileFingerprint {
    fn from(value: FileFingerprintInputDto) -> Self {
        FileFingerprint::from_parts(value.len_bytes, value.content_hash)
    }
}

impl From<FileFingerprintDto> for FileFingerprint {
    fn from(value: FileFingerprintDto) -> Self {
        FileFingerprint::from_parts(value.len_bytes, value.content_hash)
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

impl From<SyncProviderKindDto> for sync::SyncProviderKind {
    fn from(value: SyncProviderKindDto) -> Self {
        match value {
            SyncProviderKindDto::Uniseq => Self::Uniseq,
            SyncProviderKindDto::Custom => Self::Custom,
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

    fn app_config_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "app_config_unavailable",
            message: message.into(),
            path: None,
        }
    }

    fn sync(message: impl Into<String>) -> Self {
        Self {
            code: "sync_error",
            message: message.into(),
            path: None,
        }
    }

    fn ai_chat_session_missing() -> Self {
        Self {
            code: "ai_chat_session_missing",
            message: "AI chat session is no longer available".to_owned(),
            path: None,
        }
    }

    fn ai_chat_config(message: impl Into<String>) -> Self {
        Self {
            code: "ai_chat_config_invalid",
            message: message.into(),
            path: None,
        }
    }

    fn ai_chat_request(message: impl Into<String>) -> Self {
        Self {
            code: "ai_chat_request_failed",
            message: message.into(),
            path: None,
        }
    }

    fn invalid_ai_chat_message(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_ai_chat_message",
            message: message.into(),
            path: None,
        }
    }

    fn io(path: &Path, error: &std::io::Error) -> Self {
        Self {
            code: "io_error",
            message: format!("i/o error: {}", error.kind()),
            path: Some(workspace_path_to_string(path)),
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
            CoreError::InvalidWorkspaceName { message } => Self {
                code: "invalid_workspace_name",
                message,
                path: None,
            },
            CoreError::InvalidWorkspaceStructure { path, message } => Self {
                code: "invalid_workspace_structure",
                message,
                path: Some(workspace_path_to_string(&path)),
            },
            CoreError::WorkspaceParentMissing { path } => Self {
                code: "workspace_parent_missing",
                message: "workspace parent folder does not exist".to_owned(),
                path: Some(workspace_path_to_string(&path)),
            },
            CoreError::WorkspaceParentNotDirectory { path } => Self {
                code: "workspace_parent_not_directory",
                message: "workspace parent path is not a directory".to_owned(),
                path: Some(workspace_path_to_string(&path)),
            },
            CoreError::WorkspaceTargetExists { path } => Self {
                code: "workspace_target_exists",
                message: "workspace folder already exists".to_owned(),
                path: Some(workspace_path_to_string(&path)),
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
            CoreError::InvalidPageMerge => Self {
                code: "invalid_page_merge",
                message: "page merge is not valid for this source and target".to_owned(),
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
            CoreError::ConcurrentWorkspaceReconciliation => Self {
                code: "concurrent_workspace_reconciliation",
                message: "workspace reconciliation is already running in the background".to_owned(),
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

impl From<sync::SyncError> for ErrorDto {
    fn from(value: sync::SyncError) -> Self {
        Self {
            code: "sync_error",
            message: value.message().to_owned(),
            path: None,
        }
    }
}

#[tauri::command]
fn open_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    root_path: String,
) -> CommandResult<WorkspaceOpenDto> {
    {
        let mut controller = state.controller.lock().unwrap();
        let opened = controller.open_workspace(root_path)?;
        if let Err(error) = sync::disconnect_deleted_remote(Path::new(&opened.root_path)) {
            eprintln!(
                "[uniseq] failed to validate remote sync metadata for '{}': {}",
                opened.root_path,
                error.message()
            );
        }
        controller.start_sync_loop(app.clone(), Arc::clone(&state.sync_lock));
        write_last_workspace_path(&app, &opened.root_path)?;
        Ok(opened)
    }
}

#[tauri::command]
fn create_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    parent_path: String,
    folder_name: String,
) -> CommandResult<WorkspaceOpenDto> {
    let mut controller = state.controller.lock().unwrap();
    let opened = controller.create_workspace(parent_path, folder_name)?;
    controller.start_sync_loop(app.clone(), Arc::clone(&state.sync_lock));
    write_last_workspace_path(&app, &opened.root_path)?;
    Ok(opened)
}

#[tauri::command]
async fn open_remote_workspace(
    app: AppHandle,
    provider: SyncProviderKindDto,
    sync_root_url: String,
    remote_workspace_id: String,
    remote_workspace_name: String,
    auth_kind: Option<sync::SyncAuthKind>,
    auth_token: Option<String>,
    refresh_token: Option<String>,
    supabase_publishable_key: Option<String>,
    local_root_path: Option<String>,
) -> CommandResult<WorkspaceOpenDto> {
    tauri::async_runtime::spawn_blocking(move || {
        open_remote_workspace_blocking(
            app,
            provider,
            sync_root_url,
            remote_workspace_id,
            remote_workspace_name,
            auth_kind,
            auth_token,
            refresh_token,
            supabase_publishable_key,
            local_root_path,
        )
    })
    .await
    .map_err(|error| ErrorDto::sync(format!("remote workspace open task failed: {error}")))?
}

fn open_remote_workspace_blocking(
    app: AppHandle,
    provider: SyncProviderKindDto,
    sync_root_url: String,
    remote_workspace_id: String,
    remote_workspace_name: String,
    auth_kind: Option<sync::SyncAuthKind>,
    auth_token: Option<String>,
    refresh_token: Option<String>,
    supabase_publishable_key: Option<String>,
    local_root_path: Option<String>,
) -> CommandResult<WorkspaceOpenDto> {
    let config = sync::SyncConfig::new_with_auth(
        sync::SyncProviderKind::from(provider),
        sync_root_url,
        remote_workspace_id,
        remote_workspace_name,
        sync::SyncAuthConfig {
            kind: auth_kind.unwrap_or_default(),
        },
    );
    let root_path = match local_root_path.filter(|p| !p.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => remote_workspace_path(&app, &config.sync_root_url, &config.remote_workspace_id)?,
    };
    prepare_workspace_root(&root_path).map_err(ErrorDto::from)?;
    sync::write_sync_config(&root_path, &config).map_err(ErrorDto::from)?;
    sync::write_sync_auth_secrets(
        &root_path,
        &sync::SyncAuthSecrets {
            bearer_token: normalize_auth_token(auth_token),
            refresh_token: normalize_auth_token(refresh_token),
            supabase_publishable_key: normalize_auth_token(supabase_publishable_key),
        },
    )
    .map_err(ErrorDto::from)?;
    let provider = sync_provider_for_config(&root_path, &config)?;
    sync::initial_pull_with_progress(&root_path, &provider, |progress| {
        let _ = app.emit(SYNC_PROGRESS_EVENT, &progress);
    })
    .map_err(ErrorDto::from)?;
    let state = app.state::<AppState>();
    let mut controller = state.controller.lock().unwrap();
    let opened = controller.open_workspace(root_path.to_string_lossy().to_string())?;
    controller.start_sync_loop(app.clone(), Arc::clone(&state.sync_lock));
    write_last_workspace_path(&app, &opened.root_path)?;
    Ok(opened)
}

#[tauri::command]
fn close_workspace(app: AppHandle, state: State<'_, AppState>) -> bool {
    let closed = state.controller.lock().unwrap().close_workspace();
    if closed {
        let _ = clear_persisted_last_workspace_path(&app);
    }
    closed
}

#[tauri::command]
fn get_last_workspace_path(app: AppHandle) -> CommandResult<Option<String>> {
    read_last_workspace_path(&app)
}

#[tauri::command]
fn clear_last_workspace_path(app: AppHandle) -> CommandResult<bool> {
    clear_persisted_last_workspace_path(&app)
}

#[tauri::command]
fn configure_workspace_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: SyncProviderKindDto,
    sync_root_url: String,
    remote_workspace_id: String,
    remote_workspace_name: String,
    auth_kind: Option<sync::SyncAuthKind>,
    auth_token: Option<String>,
    refresh_token: Option<String>,
    supabase_publishable_key: Option<String>,
) -> CommandResult<sync::SyncStatus> {
    eprintln!(
        "[uniseq-debug] configure_workspace_sync remote_workspace_id={} remote_workspace_name={}",
        remote_workspace_id, remote_workspace_name
    );
    let mut controller = state.controller.lock().unwrap();
    controller.stop_sync_loop();
    let status = controller.configure_sync(
        provider,
        sync_root_url,
        remote_workspace_id,
        remote_workspace_name,
        auth_kind,
        auth_token,
        refresh_token,
        supabase_publishable_key,
    )?;
    controller.start_sync_loop(app, Arc::clone(&state.sync_lock));
    Ok(controller.decorate_sync_status(status))
}

#[tauri::command]
fn discover_sync_service(
    provider: SyncProviderKindDto,
    sync_root_url: String,
) -> CommandResult<sync::SyncServiceDiscovery> {
    let _provider_kind = provider;
    sync::HttpSyncProvider::discover(sync_root_url).map_err(ErrorDto::from)
}

#[tauri::command]
fn list_remote_workspaces(
    provider: SyncProviderKindDto,
    sync_root_url: String,
    auth_token: Option<String>,
) -> CommandResult<Vec<sync::RemoteWorkspace>> {
    let _provider_kind = provider;
    let provider = sync::HttpSyncProvider::new_account_with_auth(sync_root_url, auth_token)
        .map_err(ErrorDto::from)?;
    provider.list_workspaces().map_err(ErrorDto::from)
}

#[tauri::command]
fn create_remote_workspace(
    provider: SyncProviderKindDto,
    sync_root_url: String,
    workspace_name: String,
    auth_token: Option<String>,
) -> CommandResult<sync::RemoteWorkspace> {
    let _provider_kind = provider;
    let provider = sync::HttpSyncProvider::new_account_with_auth(sync_root_url, auth_token)
        .map_err(ErrorDto::from)?;
    provider
        .create_workspace(&workspace_name)
        .map_err(ErrorDto::from)
}

#[tauri::command]
fn delete_remote_workspace(
    provider: SyncProviderKindDto,
    sync_root_url: String,
    workspace_id: String,
    auth_token: Option<String>,
) -> CommandResult<bool> {
    let _provider_kind = provider;
    let provider = sync::HttpSyncProvider::new_account_with_auth(sync_root_url, auth_token)
        .map_err(ErrorDto::from)?;
    provider
        .delete_workspace(&workspace_id)
        .map_err(ErrorDto::from)?;
    Ok(true)
}

#[tauri::command]
fn delete_remote_account(
    state: State<'_, AppState>,
    provider: SyncProviderKindDto,
    sync_root_url: String,
    auth_token: Option<String>,
) -> CommandResult<bool> {
    let _provider_kind = provider;
    let sync_root_url = sync_root_url.trim().trim_end_matches('/').to_owned();
    let provider = sync::HttpSyncProvider::new_account_with_auth(sync_root_url.clone(), auth_token)
        .map_err(ErrorDto::from)?;
    provider.delete_account().map_err(ErrorDto::from)?;

    let mut controller = state.controller.lock().unwrap();
    controller.stop_sync_loop();
    if let Some(session) = controller.session.as_ref() {
        let workspace_root = session.workspace_root();
        if sync::read_sync_config(&workspace_root)
            .map_err(ErrorDto::from)?
            .as_ref()
            .is_some_and(|config| config.sync_root_url == sync_root_url)
        {
            sync::clear_sync_metadata(&workspace_root).map_err(ErrorDto::from)?;
        }
    }
    Ok(true)
}

#[tauri::command]
fn sync_status(state: State<'_, AppState>) -> CommandResult<sync::SyncStatus> {
    state.controller.lock().unwrap().sync_status()
}

#[tauri::command]
fn set_workspace_sync_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<sync::SyncStatus> {
    let mut controller = state.controller.lock().unwrap();
    let status = controller.set_sync_enabled(enabled)?;
    if enabled {
        controller.start_sync_loop(app, Arc::clone(&state.sync_lock));
    } else {
        controller.stop_sync_loop();
    }
    Ok(controller.decorate_sync_status(status))
}

#[tauri::command]
async fn sync_now(app: AppHandle) -> CommandResult<sync::SyncRunSummary> {
    tauri::async_runtime::spawn_blocking(move || sync_now_blocking(app))
        .await
        .map_err(|error| ErrorDto::sync(format!("sync task failed: {error}")))?
}

fn sync_now_blocking(app: AppHandle) -> CommandResult<sync::SyncRunSummary> {
    let state = app.state::<AppState>();
    let workspace_root = state.controller.lock().unwrap().session()?.workspace_root();
    eprintln!(
        "[uniseq-debug] sync_now start workspace_root={}",
        workspace_root.display()
    );
    let config = sync::read_sync_config(&workspace_root)
        .map_err(ErrorDto::from)?
        .ok_or_else(|| ErrorDto::sync("sync is not configured"))?;
    eprintln!(
        "[uniseq-debug] sync_now config enabled={} remote_workspace_id={}",
        config.enabled, config.remote_workspace_id
    );
    let provider = sync_provider_for_config(&workspace_root, &config)?;
    eprintln!("[uniseq-debug] sync_now waiting_for_lock");
    let mut wait_ticks = 0usize;
    let _sync_guard = loop {
        match state.sync_lock.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                wait_ticks += 1;
                let progress = sync::SyncProgress {
                    operation: sync::SyncProgressOperation::Sync,
                    phase: sync::SyncProgressPhase::Waiting,
                    current: 0,
                    total: 0,
                    path: None,
                    detail: Some("Waiting for current sync to finish".to_owned()),
                };
                eprintln!("[uniseq-debug] sync_now waiting_for_lock tick={wait_ticks}");
                if let Err(error) = app.emit(SYNC_PROGRESS_EVENT, &progress) {
                    eprintln!("[uniseq-debug] sync_progress waiting_emit_error={error}");
                }
                thread::sleep(Duration::from_millis(500));
            }
            Err(std::sync::TryLockError::Poisoned(error)) => {
                return Err(ErrorDto::sync(format!("sync lock poisoned: {error}")));
            }
        }
    };
    eprintln!("[uniseq-debug] sync_now acquired_lock");
    let result = sync::sync_once_with_progress(&workspace_root, &provider, |progress| {
        eprintln!(
            "[uniseq-debug] sync_progress emit operation={:?} phase={:?} current={} total={} path={:?} detail={:?}",
            progress.operation,
            progress.phase,
            progress.current,
            progress.total,
            progress.path,
            progress.detail
        );
        if let Err(error) = app.emit(SYNC_PROGRESS_EVENT, &progress) {
            eprintln!("[uniseq-debug] sync_progress emit_error={error}");
        }
    });
    let summary = match result {
        Err(ref e) if e.auth_expired => {
            let secrets = sync::read_sync_auth_secrets(&workspace_root).map_err(ErrorDto::from)?;
            let new_secrets = sync::refresh_supabase_auth(&config.sync_root_url, &secrets)
                .map_err(ErrorDto::from)?;
            sync::write_sync_auth_secrets(&workspace_root, &new_secrets).map_err(ErrorDto::from)?;
            let new_provider = sync_provider_for_config(&workspace_root, &config)?;
            let refreshed = sync::sync_once_with_progress(
                &workspace_root,
                &new_provider,
                |progress| {
                    eprintln!(
                        "[uniseq-debug] sync_progress emit_after_refresh operation={:?} phase={:?} current={} total={} path={:?} detail={:?}",
                        progress.operation,
                        progress.phase,
                        progress.current,
                        progress.total,
                        progress.path,
                        progress.detail
                    );
                    if let Err(error) = app.emit(SYNC_PROGRESS_EVENT, &progress) {
                        eprintln!("[uniseq-debug] sync_progress emit_after_refresh_error={error}");
                    }
                },
            );
            match refreshed {
                Err(error) if error.remote_missing => {
                    sync::clear_sync_metadata(&workspace_root).map_err(ErrorDto::from)?;
                    sync::SyncRunSummary {
                        pushed: 0,
                        pulled: 0,
                        deleted_local: 0,
                        deleted_remote: 0,
                        conflicts: Vec::new(),
                        status: sync::sync_status(&workspace_root).map_err(ErrorDto::from)?,
                    }
                }
                other => other.map_err(ErrorDto::from)?,
            }
        }
        Err(error) if error.remote_missing => {
            sync::clear_sync_metadata(&workspace_root).map_err(ErrorDto::from)?;
            sync::SyncRunSummary {
                pushed: 0,
                pulled: 0,
                deleted_local: 0,
                deleted_remote: 0,
                conflicts: Vec::new(),
                status: sync::sync_status(&workspace_root).map_err(ErrorDto::from)?,
            }
        }
        other => other.map_err(ErrorDto::from)?,
    };
    let summary = state
        .controller
        .lock()
        .unwrap()
        .decorate_sync_summary(summary);
    drop(_sync_guard);
    eprintln!(
        "[uniseq-debug] sync_now complete pushed={} pulled={} deleted_local={} deleted_remote={} conflicts={}",
        summary.pushed,
        summary.pulled,
        summary.deleted_local,
        summary.deleted_remote,
        summary.conflicts.len()
    );
    let _ = app.emit("sync-status", &summary.status);
    if summary.pulled > 0 || summary.deleted_local > 0 {
        state.controller.lock().unwrap().reopen_workspace()?;
    }
    Ok(summary)
}

#[tauri::command]
fn open_ai_chat_session(
    state: State<'_, AppState>,
    context_spec: AiChatContextSpecDto,
) -> CommandResult<AiChatSessionOpenDto> {
    state
        .controller
        .lock()
        .unwrap()
        .open_ai_chat_session(context_spec)
}

#[tauri::command]
fn close_ai_chat_session(state: State<'_, AppState>, session_id: String) -> bool {
    state
        .controller
        .lock()
        .unwrap()
        .close_ai_chat_session(session_id)
}

#[tauri::command]
async fn ai_chat(
    state: State<'_, AppState>,
    session_id: String,
    prior_messages: Vec<AiChatMessageDto>,
    latest_user_message: String,
    api_key: String,
) -> CommandResult<AiChatResponseDto> {
    let latest_user_message = latest_user_message.trim().to_owned();
    if latest_user_message.is_empty() {
        return Err(ErrorDto::invalid_ai_chat_message(
            "latest user message cannot be empty",
        ));
    }
    let api_key = api_key.trim().to_owned();
    if api_key.is_empty() {
        return Err(ErrorDto::ai_chat_config("Gemini API key is required"));
    }

    let system_prompt = {
        let controller = state.controller.lock().unwrap();
        let active_session = controller.active_ai_chat_session(&session_id)?;
        active_session.system_prompt.clone()
    };

    tauri::async_runtime::spawn_blocking(move || {
        let prior_messages = prior_messages
            .into_iter()
            .map(|message| ai::ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect::<Vec<_>>();
        ai::chat_completion(
            &system_prompt,
            &prior_messages,
            &latest_user_message,
            &api_key,
        )
            .map(|assistant_text| AiChatResponseDto { assistant_text })
            .map_err(|error| match error {
                ai::ChatError::Config(message) => ErrorDto::ai_chat_config(message),
                ai::ChatError::Request(message) => ErrorDto::ai_chat_request(message),
            })
    })
    .await
    .map_err(|error| ErrorDto::ai_chat_request(format!("AI chat task failed: {error}")))?
}

#[tauri::command]
fn notify_user_activity(state: State<'_, AppState>) {
    state.controller.lock().unwrap().notify_user_activity();
}

#[tauri::command]
fn sync_conflict_detail(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<sync::SyncConflictDetail> {
    state.controller.lock().unwrap().sync_conflict_detail(path)
}

#[tauri::command]
fn resolve_sync_conflict(
    state: State<'_, AppState>,
    path: String,
    resolution: sync::SyncConflictResolution,
) -> CommandResult<sync::SyncRunSummary> {
    state
        .controller
        .lock()
        .unwrap()
        .resolve_sync_conflict(path, resolution)
}

#[tauri::command]
fn all_pages(state: State<'_, AppState>) -> CommandResult<Vec<PageSummaryDto>> {
    state.controller.lock().unwrap().all_pages()
}

#[tauri::command]
fn find_empty_pages(state: State<'_, AppState>) -> CommandResult<Vec<PageSummaryDto>> {
    state.controller.lock().unwrap().find_empty_pages()
}

#[tauri::command]
fn all_streams(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state.controller.lock().unwrap().all_streams()
}

#[tauri::command]
fn page_order(state: State<'_, AppState>, app: AppHandle) -> CommandResult<PageOrderDto> {
    state.controller.lock().unwrap().page_order(&app)
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
fn page_content(state: State<'_, AppState>, page_id: String) -> CommandResult<PageContentDto> {
    state.controller.lock().unwrap().page_content(page_id)
}

#[tauri::command]
fn search_pages(
    state: State<'_, AppState>,
    page_query: String,
    limit: Option<usize>,
) -> CommandResult<Vec<SearchResultDto>> {
    state
        .controller
        .lock()
        .unwrap()
        .search_pages(page_query, limit)
}

#[tauri::command]
fn write_page_content(
    state: State<'_, AppState>,
    page_id: String,
    text: String,
    expected_revision: Option<FileFingerprintInputDto>,
) -> CommandResult<PageContentDto> {
    state
        .controller
        .lock()
        .unwrap()
        .write_page_content(page_id, text, expected_revision)
}

#[tauri::command]
fn create_stream_page(
    state: State<'_, AppState>,
    stream_name: String,
    date_name: String,
) -> CommandResult<PageSummaryDto> {
    state
        .controller
        .lock()
        .unwrap()
        .create_stream_page(stream_name, date_name)
}

#[tauri::command]
fn delete_stream_page(
    state: State<'_, AppState>,
    stream_name: String,
    date_name: String,
) -> CommandResult<()> {
    state
        .controller
        .lock()
        .unwrap()
        .delete_stream_page(stream_name, date_name)
}

#[tauri::command]
fn delete_stream(
    state: State<'_, AppState>,
    stream_name: String,
) -> CommandResult<DeleteStreamResultDto> {
    state.controller.lock().unwrap().delete_stream(stream_name)
}

#[tauri::command]
fn rename_stream(
    state: State<'_, AppState>,
    stream_name: String,
    new_stream_name: String,
) -> CommandResult<()> {
    state
        .controller
        .lock()
        .unwrap()
        .rename_stream(stream_name, new_stream_name)
}

#[tauri::command]
fn cleanup_empty_stream_pages(
    state: State<'_, AppState>,
    older_than_days: u64,
) -> CommandResult<CleanupResultDto> {
    state
        .controller
        .lock()
        .unwrap()
        .cleanup_empty_stream_pages(older_than_days)
}

#[tauri::command]
fn refresh_stream_workspace(
    state: State<'_, AppState>,
    older_than_days: u64,
) -> CommandResult<CleanupResultDto> {
    state
        .controller
        .lock()
        .unwrap()
        .refresh_stream_workspace(older_than_days)
}

#[tauri::command]
fn create_page(state: State<'_, AppState>, page_id: String) -> CommandResult<()> {
    state.controller.lock().unwrap().create_page(page_id)
}

#[tauri::command]
fn rename_page(
    state: State<'_, AppState>,
    app: AppHandle,
    page_id: String,
    new_title: String,
) -> CommandResult<()> {
    state
        .controller
        .lock()
        .unwrap()
        .rename_page(&app, page_id, new_title)
}

#[tauri::command]
fn move_page(
    state: State<'_, AppState>,
    app: AppHandle,
    page_id: String,
    new_parent_page_id: Option<String>,
) -> CommandResult<()> {
    state
        .controller
        .lock()
        .unwrap()
        .move_page(&app, page_id, new_parent_page_id)
}

#[tauri::command]
fn delete_page(state: State<'_, AppState>, app: AppHandle, page_id: String) -> CommandResult<()> {
    state.controller.lock().unwrap().delete_page(&app, page_id)
}

#[tauri::command]
fn merge_page(
    state: State<'_, AppState>,
    app: AppHandle,
    source_page_id: String,
    target_page_id: String,
) -> CommandResult<()> {
    state
        .controller
        .lock()
        .unwrap()
        .merge_page(&app, source_page_id, target_page_id)
}

#[tauri::command]
fn set_page_sibling_order(
    state: State<'_, AppState>,
    app: AppHandle,
    parent_page_id: Option<String>,
    ordered_child_page_ids: Vec<String>,
) -> CommandResult<()> {
    state.controller.lock().unwrap().set_page_sibling_order(
        &app,
        parent_page_id,
        ordered_child_page_ids,
    )
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
fn page_linked_refs(
    state: State<'_, AppState>,
    page_id: String,
) -> CommandResult<Vec<LinkedRefEntryDto>> {
    state.controller.lock().unwrap().page_linked_refs(page_id)
}

#[tauri::command]
fn block_snapshot(
    state: State<'_, AppState>,
    handle: BlockHandleDto,
) -> CommandResult<BlockSnapshotDto> {
    state.controller.lock().unwrap().block_snapshot(handle)
}

#[tauri::command]
fn write_block_markdown(
    state: State<'_, AppState>,
    handle: BlockHandleDto,
    replacement_markdown: String,
) -> CommandResult<BlockSnapshotDto> {
    state
        .controller
        .lock()
        .unwrap()
        .write_block_markdown(handle, replacement_markdown)
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

#[tauri::command]
fn open_url(url: String) {
    let _ = open::that(url);
}

#[tauri::command]
fn get_default_workspace_path(app: AppHandle) -> CommandResult<String> {
    let workspace_path = default_workspace_path(&app)?;
    Ok(workspace_path.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
fn open_or_create_default_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<WorkspaceOpenDto> {
    let parent_path = workspace_storage_dir(&app)?;
    fs::create_dir_all(&parent_path).map_err(|error| ErrorDto::io(&parent_path, &error))?;

    let default_root = parent_path.join(DEFAULT_WORKSPACE_FOLDER_NAME);
    let mut controller = state.controller.lock().unwrap();
    let opened = if default_root.join("pages").is_dir() {
        controller.open_workspace(default_root.to_string_lossy().to_string())?
    } else {
        controller.create_workspace(
            parent_path.to_string_lossy().to_string(),
            DEFAULT_WORKSPACE_FOLDER_NAME.to_owned(),
        )?
    };
    controller.start_sync_loop(app.clone(), Arc::clone(&state.sync_lock));
    write_last_workspace_path(&app, &opened.root_path)?;
    Ok(opened)
}

// Swift implementation required in the Xcode project (src-tauri/gen/apple):
//
//   @_cdecl("uniseq_icloud_container_path")
//   func icloudContainerPath(buf: UnsafeMutablePointer<UInt8>, len: Int) -> Int {
//       guard let url = FileManager.default
//           .url(forUbiquityContainerIdentifier: nil)?
//           .appendingPathComponent("Documents") else { return 0 }
//       let bytes = Array(url.path.utf8)
//       let count = min(bytes.count, len)
//       buf.initialize(from: bytes, count: count)
//       return count
//   }
//
// Also requires iCloud Documents entitlement and NSUbiquitousContainers in Info.plist.
#[cfg(target_os = "ios")]
extern "C" {
    fn uniseq_icloud_container_path(buf: *mut u8, len: usize) -> usize;
}

#[cfg(target_os = "ios")]
fn resolve_icloud_container_path() -> CommandResult<String> {
    let mut buf = vec![0u8; 4096];
    let len = unsafe { uniseq_icloud_container_path(buf.as_mut_ptr(), buf.len()) };
    if len == 0 {
        return Err(ErrorDto::app_config_unavailable(
            "iCloud is not available on this device".to_string(),
        ));
    }
    buf.truncate(len);
    String::from_utf8(buf).map_err(|_| {
        ErrorDto::app_config_unavailable("iCloud path contains invalid UTF-8".to_string())
    })
}

#[tauri::command]
fn open_icloud_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_name: String,
) -> CommandResult<WorkspaceOpenDto> {
    #[cfg(target_os = "ios")]
    {
        let container_path = resolve_icloud_container_path()?;
        let mut controller = state.controller.lock().unwrap();
        let opened = controller.create_workspace(container_path, folder_name)?;
        controller.start_sync_loop(app.clone(), Arc::clone(&state.sync_lock));
        write_last_workspace_path(&app, &opened.root_path)?;
        Ok(opened)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, state, folder_name);
        Err(ErrorDto::app_config_unavailable(
            "iCloud workspace is only available on iOS".to_string(),
        ))
    }
}

fn remote_workspace_path(
    app: &AppHandle,
    sync_root_url: &str,
    remote_workspace_id: &str,
) -> CommandResult<PathBuf> {
    Ok(
        workspace_storage_dir(app)?.join(safe_remote_folder_name(&format!(
            "{sync_root_url}/{remote_workspace_id}"
        ))),
    )
}

fn workspace_storage_dir(app: &AppHandle) -> CommandResult<PathBuf> {
    let data_dir = app.path().app_data_dir().map_err(|error| {
        ErrorDto::app_config_unavailable(format!("failed to resolve app data directory: {error}"))
    })?;
    Ok(workspace_storage_dir_from_app_data(&data_dir))
}

fn default_workspace_path(app: &AppHandle) -> CommandResult<PathBuf> {
    Ok(default_workspace_path_from_storage_root(
        &workspace_storage_dir(app)?,
    ))
}

fn workspace_storage_dir_from_app_data(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("workspace")
}

fn default_workspace_path_from_storage_root(storage_root: &Path) -> PathBuf {
    storage_root.join(DEFAULT_WORKSPACE_FOLDER_NAME)
}

fn safe_remote_folder_name(sync_root_url: &str) -> String {
    let mut output = String::new();
    for ch in sync_root_url.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') {
            output.push(ch);
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "remote".to_owned()
    } else {
        output.chars().take(80).collect()
    }
}

fn sync_provider_for_config(
    workspace_root: &Path,
    config: &sync::SyncConfig,
) -> CommandResult<sync::HttpSyncProvider> {
    let secrets = sync::read_sync_auth_secrets(workspace_root).map_err(ErrorDto::from)?;
    sync::HttpSyncProvider::new_workspace_with_auth(
        config.sync_root_url.clone(),
        config.remote_workspace_id.clone(),
        secrets.bearer_token,
    )
    .map_err(ErrorDto::from)
}

fn normalize_auth_token(token: Option<String>) -> Option<String> {
    token
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            use tauri_plugin_deep_link::DeepLinkExt;
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    let _ = handle.emit("deep-link-url", url.to_string());
                }
            });
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            create_workspace,
            open_remote_workspace,
            close_workspace,
            get_last_workspace_path,
            clear_last_workspace_path,
            configure_workspace_sync,
            discover_sync_service,
            list_remote_workspaces,
            create_remote_workspace,
            delete_remote_workspace,
            delete_remote_account,
            sync_status,
            set_workspace_sync_enabled,
            sync_now,
            open_ai_chat_session,
            close_ai_chat_session,
            ai_chat,
            notify_user_activity,
            sync_conflict_detail,
            resolve_sync_conflict,
            all_pages,
            find_empty_pages,
            all_streams,
            page_order,
            page_summary,
            page_detail,
            page_content,
            search_pages,
            write_page_content,
            create_page,
            rename_page,
            move_page,
            delete_page,
            merge_page,
            set_page_sibling_order,
            page_incoming_refs,
            page_outgoing_refs,
            page_linked_refs,
            block_snapshot,
            write_block_markdown,
            drain_workspace_events,
            take_last_watch_error,
            start_watching,
            stop_watching,
            open_url,
            get_default_workspace_path,
            open_or_create_default_workspace,
            open_icloud_workspace,
            create_stream_page,
            delete_stream,
            delete_stream_page,
            rename_stream,
            cleanup_empty_stream_pages,
            refresh_stream_workspace
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}

fn build_ai_chat_session(
    session: &WorkspaceSession,
    context_spec: &AiChatContextSpecDto,
    char_budget: usize,
) -> CommandResult<PreparedAiChatSession> {
    match context_spec {
        AiChatContextSpecDto::Page { page_id } => {
            let page_id =
                parse_page_id_input(page_id).map_err(|_| ErrorDto::invalid_page_id(page_id))?;
            build_page_ai_chat_session(session, &page_id, char_budget)
        }
        AiChatContextSpecDto::StreamSingle { stream_name } => {
            build_stream_ai_chat_session(session, &[stream_name.clone()], char_budget)
        }
        AiChatContextSpecDto::StreamDual { stream_names } => {
            if stream_names.is_empty() {
                return Err(ErrorDto::invalid_ai_chat_message(
                    "dual-stream AI chat requires at least one stream name",
                ));
            }
            build_stream_ai_chat_session(session, stream_names, char_budget)
        }
    }
}

fn build_page_ai_chat_session(
    session: &WorkspaceSession,
    page_id: &PageId,
    char_budget: usize,
) -> CommandResult<PreparedAiChatSession> {
    let all_pages = session.all_pages();
    let page_summary = session.page_summary(page_id).map_err(ErrorDto::from)?;
    let page_content = session.page_content(page_id).map_err(ErrorDto::from)?;
    let linked_refs = session.page_linked_refs(page_id).map_err(ErrorDto::from)?;
    let mut remaining_chars = char_budget;
    let mut context_body = String::new();
    let mut truncated = false;
    let mut included_page_ids = vec![page_id_to_string(page_id)];
    let mut preview_related_page_titles = Vec::<String>::new();
    let mut preview_stream_dates_by_name = BTreeMap::<String, Vec<String>>::new();
    let page_section = format!(
        "{}\n\n{}\n",
        display_page_title(&page_summary.title, page_id),
        page_content.text.trim()
    );
    if !push_fragment_with_optional_truncation(
        &mut context_body,
        &page_section,
        &mut remaining_chars,
        true,
    ) {
        truncated = true;
    } else {
        for linked_ref in linked_refs {
            let source_title = related_note_label(&all_pages, &linked_ref.source_page_id)
                .unwrap_or_else(|| page_id_to_string(&linked_ref.source_page_id));
            let source_id = page_id_to_string(&linked_ref.source_page_id);
            let source_block = if linked_ref.block.markdown.trim().is_empty() {
                linked_ref.block.content.trim().to_owned()
            } else {
                linked_ref.block.markdown.trim().to_owned()
            };
            let fragment = format!(
                "\nRelated note: {}\n\n{}\n",
                source_title, source_block
            );
            if !push_fragment_with_optional_truncation(
                &mut context_body,
                &fragment,
                &mut remaining_chars,
                false,
            ) {
                truncated = true;
                break;
            }
            included_page_ids.push(source_id);
            match linked_ref.source_page_id.location() {
                PageLocation::Pages => {
                    if !preview_related_page_titles.iter().any(|title| title == &source_title) {
                        preview_related_page_titles.push(source_title);
                    }
                }
                PageLocation::Stream { stream_name } => {
                    let entry = preview_stream_dates_by_name
                        .entry(stream_name.as_str().to_owned())
                        .or_default();
                    let date_name = linked_ref.source_page_id.leaf_name().as_str().to_owned();
                    if !entry.iter().any(|existing| existing == &date_name) {
                        entry.push(date_name);
                    }
                }
            }
        }
    }

    let preview_related_note_sources = summarize_related_note_sources(
        &preview_related_page_titles,
        &preview_stream_dates_by_name,
    );
    let preview_summary = if preview_related_note_sources.is_empty() {
        format!(
            "Start chat based on {}.",
            display_page_title(&page_summary.title, page_id)
        )
    } else {
        format!(
            "Start chat based on {} and related notes from {}.",
            display_page_title(&page_summary.title, page_id),
            human_join(&preview_related_note_sources)
        )
    };
    let system_prompt = build_ai_system_prompt(
        &preview_summary,
        &context_body,
        Some(
            "The main page content should be treated as relatively time-agnostic reference information. Related notes may add dated context around it.",
        ),
    );
    Ok(PreparedAiChatSession {
        system_prompt,
        preview_summary,
        truncated,
        included_page_ids,
    })
}

fn build_stream_ai_chat_session(
    session: &WorkspaceSession,
    stream_names: &[String],
    char_budget: usize,
) -> CommandResult<PreparedAiChatSession> {
    let mut ordered_pages = session
        .all_pages()
        .into_iter()
        .filter(|page| match &page.location {
            PageLocation::Stream { stream_name } => {
                stream_names.iter().any(|value| value == stream_name.as_str())
            }
            PageLocation::Pages => false,
        })
        .collect::<Vec<_>>();
    ordered_pages.sort_by(|left, right| compare_stream_pages(left, right, stream_names));

    let mut remaining_chars = char_budget;
    let mut truncated = false;
    let mut context_body = String::new();
    let mut included_page_ids = Vec::<String>::new();
    let mut included_dates = Vec::<String>::new();
    for page in ordered_pages {
        let page_id = page_id_to_string(&page.page_id);
        let stream_name = read_stream_name_from_location(&page.location)
            .unwrap_or_default()
            .to_owned();
        let date_name = page.page_id.leaf_name().as_str().to_owned();
        let content = session.page_content(&page.page_id).map_err(ErrorDto::from)?;
        let fragment = format!(
            "{}\n{}\n\n{}\n\n",
            format_date_name(&date_name),
            stream_name,
            content.text.trim()
        );
        if !push_fragment_with_optional_truncation(
            &mut context_body,
            &fragment,
            &mut remaining_chars,
            false,
        ) {
            truncated = true;
            break;
        }
        included_dates.push(date_name);
        included_page_ids.push(page_id);
    }

    let preview_summary = build_stream_preview_summary(stream_names, &included_dates);
    let system_prompt = build_ai_system_prompt(&preview_summary, &context_body, None);
    Ok(PreparedAiChatSession {
        system_prompt,
        preview_summary,
        truncated,
        included_page_ids,
    })
}

fn build_ai_system_prompt(
    preview_summary: &str,
    context_body: &str,
    context_guidance: Option<&str>,
) -> String {
    format!(
        "You are chatting with the owner of the notes below.\n\
Use the notes as context for the conversation.\n\
The notes belong the the owner but mind that some contents might be external, rather than written by the owner themselves.\n\
Make useful links between the user's question and what can be inferred from the notes.\n\
Pay attention to chronology and how ideas develop over time.\n\
If the notes do not support a claim, say so clearly.\n\
{}\n\n\
Context: {preview_summary}\n\n\
Notes:\n{context_body}",
        context_guidance.unwrap_or("")
    )
}

fn compare_stream_pages(
    left: &PageSummary,
    right: &PageSummary,
    stream_names: &[String],
) -> std::cmp::Ordering {
    let left_date = left.page_id.leaf_name().as_str();
    let right_date = right.page_id.leaf_name().as_str();
    right_date
        .cmp(left_date)
        .then_with(|| {
            let left_index = read_stream_name_from_location(&left.location)
                .and_then(|stream_name| stream_names.iter().position(|name| name == stream_name))
                .unwrap_or(usize::MAX);
            let right_index = read_stream_name_from_location(&right.location)
                .and_then(|stream_name| stream_names.iter().position(|name| name == stream_name))
                .unwrap_or(usize::MAX);
            left_index.cmp(&right_index)
        })
        .then_with(|| page_id_to_string(&left.page_id).cmp(&page_id_to_string(&right.page_id)))
}

fn read_stream_name_from_location(location: &PageLocation) -> Option<&str> {
    match location {
        PageLocation::Stream { stream_name } => Some(stream_name.as_str()),
        PageLocation::Pages => None,
    }
}

fn build_stream_preview_summary(stream_names: &[String], included_dates: &[String]) -> String {
    let scope = human_join(stream_names);
    let Some(newest) = included_dates.first() else {
        return format!("Start chat based on {}.", scope);
    };
    let oldest = included_dates.last().unwrap_or(newest);
    if newest == oldest {
        format!(
            "Start chat based on {} on {}.",
            scope,
            format_date_name(newest)
        )
    } else {
        format!(
            "Start chat based on {} from {} to {}.",
            scope,
            format_date_name(oldest),
            format_date_name(newest)
        )
    }
}

fn summarize_related_note_sources(
    page_titles: &[String],
    stream_dates_by_name: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut items = page_titles.to_vec();
    for (stream_name, date_names) in stream_dates_by_name {
        if date_names.is_empty() {
            continue;
        }
        let mut sorted_dates = date_names.to_vec();
        sorted_dates.sort();
        let oldest = sorted_dates.first().unwrap();
        let newest = sorted_dates.last().unwrap();
        if oldest == newest {
            items.push(format!("{stream_name} on {}", format_date_name(newest)));
        } else {
            items.push(format!(
                "{stream_name} from {} to {}",
                format_date_name(oldest),
                format_date_name(newest)
            ));
        }
    }
    items
}

fn display_page_title(title: &str, page_id: &PageId) -> String {
    if title.trim().is_empty() {
        page_id_to_string(page_id)
    } else {
        title.to_owned()
    }
}

fn related_note_label(all_pages: &[PageSummary], page_id: &PageId) -> Option<String> {
    let page = all_pages.iter().find(|page| page.page_id == *page_id)?;
    let title = display_page_title(&page.title, &page.page_id);
    let stream_name = read_stream_name_from_location(&page.location)?;
    Some(format!("{title} ({stream_name})"))
}

fn push_fragment_with_optional_truncation(
    target: &mut String,
    fragment: &str,
    remaining_chars: &mut usize,
    allow_truncation: bool,
) -> bool {
    let fragment_chars = fragment.chars().count();
    if fragment_chars <= *remaining_chars {
        target.push_str(fragment);
        *remaining_chars -= fragment_chars;
        return true;
    }
    if allow_truncation && *remaining_chars > 0 {
        target.push_str(&prefix_chars(fragment, *remaining_chars));
        *remaining_chars = 0;
    }
    false
}

fn prefix_chars(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    match value.char_indices().nth(max_chars) {
        Some((index, _)) => value[..index].to_owned(),
        None => value.to_owned(),
    }
}

fn human_join(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [only] => only.clone(),
        [left, right] => format!("{left} and {right}"),
        _ => {
            let mut joined = values[..values.len() - 1].join(", ");
            joined.push_str(", and ");
            joined.push_str(values.last().unwrap());
            joined
        }
    }
}

fn ai_context_char_budget() -> usize {
    AI_CHAT_CONTEXT_TOKEN_BUDGET.saturating_mul(AI_CHAT_APPROX_CHARS_PER_TOKEN)
}

fn format_date_name(date_name: &str) -> String {
    date_name.replace('_', "-")
}

fn parse_page_id_input(input: &str) -> Result<PageId, ()> {
    if let Some(page_path) = input.strip_prefix("pages:") {
        return PageId::new(page_path.split('/')).map_err(|_| ());
    }

    if let Some(stream_path) = input
        .strip_prefix("stream:")
        .or_else(|| input.strip_prefix("streams/"))
    {
        let mut segments = stream_path.split('/');
        let Some(stream_name) = segments.next() else {
            return Err(());
        };
        let Some(date_name) = segments.next() else {
            return Err(());
        };
        if segments.next().is_some() {
            return Err(());
        }

        let stream_name = PageName::new(stream_name).map_err(|_| ())?;
        let date_name = PageName::new(date_name).map_err(|_| ())?;
        return PageId::stream(stream_name, date_name).map_err(|_| ());
    }

    PageId::from_str(input).map_err(|_| ())
}

fn page_id_to_string(page_id: &PageId) -> String {
    page_id.canonical_identity_display()
}

fn workspace_path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn workspace_root_key(path: &Path) -> String {
    workspace_path_to_string(path)
}

fn parent_order_key(parent_page_id: Option<&str>) -> String {
    parent_page_id.unwrap_or(ROOT_PARENT_ORDER_KEY).to_owned()
}

fn renamed_page_id_string(page_id: &PageId, new_leaf_name: &PageName) -> String {
    let mut segments = page_id
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_owned())
        .collect::<Vec<_>>();
    if let Some(last) = segments.last_mut() {
        *last = new_leaf_name.as_str().to_owned();
    }
    format!("pages:{}", segments.join("/"))
}

fn moved_page_id_string(page_id: &PageId, destination_parent_page_id: Option<&PageId>) -> String {
    let leaf_name = page_id.leaf_name().as_str();
    destination_parent_page_id
        .map(|parent| format!("{}/{}", page_id_to_string(parent), leaf_name))
        .unwrap_or_else(|| format!("pages:{leaf_name}"))
}

fn days_since_date_name(date_name: &str) -> Option<u64> {
    let bytes = date_name.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'_' || bytes[7] != b'_' {
        return None;
    }
    let year: i64 = std::str::from_utf8(&bytes[0..4]).ok()?.parse().ok()?;
    let month: i64 = std::str::from_utf8(&bytes[5..7]).ok()?.parse().ok()?;
    let day: i64 = std::str::from_utf8(&bytes[8..10]).ok()?.parse().ok()?;
    // Julian Day Number
    let a = (14 - month) / 12;
    let y = year + 4800 - a;
    let m = month + 12 * a - 3;
    let jdn = day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    // Unix epoch 1970-01-01 = JDN 2440588
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let today_jdn = (secs / 86400) as i64 + 2440588;
    let age = today_jdn - jdn;
    (age >= 0).then_some(age as u64)
}

fn workspace_page_order_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("uniseq").join(PAGE_ORDER_FILE_NAME)
}

fn old_page_order_store_path(app: &AppHandle) -> CommandResult<PathBuf> {
    Ok(app_storage_dir(app)?.join(OLD_PAGE_ORDER_STORE_FILE_NAME))
}

fn read_workspace_page_order(
    app: &AppHandle,
    workspace_root: &Path,
) -> CommandResult<WorkspacePageOrder> {
    let path = workspace_page_order_path(workspace_root);
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(|error| ErrorDto {
            code: "page_order_store_invalid",
            message: format!("failed to parse workspace page order: {error}"),
            path: Some(workspace_path_to_string(&path)),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let old_store = read_old_page_order_store(app)?;
            Ok(old_store
                .workspaces
                .get(&workspace_root_key(workspace_root))
                .cloned()
                .unwrap_or_default())
        }
        Err(error) => Err(ErrorDto::io(&path, &error)),
    }
}

fn write_workspace_page_order(
    workspace_root: &Path,
    workspace_order: &WorkspacePageOrder,
) -> CommandResult<()> {
    let path = workspace_page_order_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ErrorDto::io(parent, &error))?;
    }
    let contents = serde_json::to_string_pretty(workspace_order).map_err(|error| ErrorDto {
        code: "page_order_store_invalid",
        message: format!("failed to serialize workspace page order: {error}"),
        path: Some(workspace_path_to_string(&path)),
    })?;
    fs::write(&path, contents).map_err(|error| ErrorDto::io(&path, &error))
}

fn read_old_page_order_store(app: &AppHandle) -> CommandResult<PageOrderStore> {
    let path = old_page_order_store_path(app)?;
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(|error| ErrorDto {
            code: "page_order_store_invalid",
            message: format!("failed to parse page order store: {error}"),
            path: Some(workspace_path_to_string(&path)),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PageOrderStore::default()),
        Err(error) => Err(ErrorDto::io(&path, &error)),
    }
}

fn normalize_workspace_page_order(
    workspace_order: &WorkspacePageOrder,
    pages: &[PageSummary],
) -> WorkspacePageOrder {
    let mut child_ids_by_parent = BTreeMap::<String, Vec<String>>::new();
    for page in pages
        .iter()
        .filter(|page| matches!(page.location, PageLocation::Pages))
    {
        let parent_page_id = page.parent_page_id.as_ref().map(page_id_to_string);
        child_ids_by_parent
            .entry(parent_order_key(parent_page_id.as_deref()))
            .or_default()
            .push(page_id_to_string(&page.page_id));
    }

    for child_ids in child_ids_by_parent.values_mut() {
        child_ids.sort();
    }

    let mut sibling_order_by_parent = BTreeMap::new();
    for (parent_key, child_ids) in child_ids_by_parent {
        let child_set = child_ids.iter().cloned().collect::<BTreeSet<_>>();
        let mut normalized = workspace_order
            .sibling_order_by_parent
            .get(&parent_key)
            .into_iter()
            .flatten()
            .filter(|page_id| child_set.contains(*page_id))
            .cloned()
            .collect::<Vec<_>>();
        let existing = normalized.iter().cloned().collect::<BTreeSet<_>>();
        normalized.extend(
            child_ids
                .into_iter()
                .filter(|page_id| !existing.contains(page_id)),
        );
        sibling_order_by_parent.insert(parent_key, normalized);
    }

    WorkspacePageOrder {
        sibling_order_by_parent,
    }
}

fn remap_workspace_page_order_subtree(
    workspace_order: &mut WorkspacePageOrder,
    source_page_id: &str,
    target_page_id: &str,
) {
    let mut remapped = BTreeMap::<String, Vec<String>>::new();
    for (parent_key, sibling_ids) in &workspace_order.sibling_order_by_parent {
        let remapped_parent_key =
            remap_parent_order_key(parent_key, source_page_id, target_page_id);
        let entry = remapped.entry(remapped_parent_key).or_default();
        for sibling_id in sibling_ids {
            let remapped_sibling_id =
                remap_subtree_page_id(sibling_id, source_page_id, target_page_id);
            if !entry.contains(&remapped_sibling_id) {
                entry.push(remapped_sibling_id);
            }
        }
    }
    workspace_order.sibling_order_by_parent = remapped;
}

fn remove_workspace_page_order_subtree(
    workspace_order: &mut WorkspacePageOrder,
    source_page_id: &str,
) {
    workspace_order.sibling_order_by_parent = workspace_order
        .sibling_order_by_parent
        .iter()
        .filter_map(|(parent_key, sibling_ids)| {
            if is_page_in_subtree(parent_key, source_page_id) {
                return None;
            }
            let filtered = sibling_ids
                .iter()
                .filter(|page_id| !is_page_in_subtree(page_id, source_page_id))
                .cloned()
                .collect::<Vec<_>>();
            Some((parent_key.clone(), filtered))
        })
        .collect();
}

fn remap_parent_order_key(parent_key: &str, source_page_id: &str, target_page_id: &str) -> String {
    if parent_key == ROOT_PARENT_ORDER_KEY {
        return parent_key.to_owned();
    }
    remap_subtree_page_id(parent_key, source_page_id, target_page_id)
}

fn remap_subtree_page_id(page_id: &str, source_page_id: &str, target_page_id: &str) -> String {
    if page_id == source_page_id {
        return target_page_id.to_owned();
    }
    let prefix = format!("{source_page_id}/");
    if let Some(rest) = page_id.strip_prefix(&prefix) {
        return format!("{target_page_id}/{rest}");
    }
    page_id.to_owned()
}

fn is_page_in_subtree(page_id: &str, root_page_id: &str) -> bool {
    page_id == root_page_id || page_id.starts_with(&format!("{root_page_id}/"))
}

fn app_storage_dir(app: &AppHandle) -> CommandResult<PathBuf> {
    app.path().app_config_dir().map_err(|error| {
        ErrorDto::app_config_unavailable(format!("failed to resolve app config directory: {error}"))
    })
}

fn last_workspace_path_file(app: &AppHandle) -> CommandResult<PathBuf> {
    Ok(app_storage_dir(app)?.join(LAST_WORKSPACE_FILE_NAME))
}

fn read_last_workspace_path(app: &AppHandle) -> CommandResult<Option<String>> {
    let path = last_workspace_path_file(app)?;
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let trimmed = contents.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_owned()))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ErrorDto::io(&path, &error)),
    }
}

fn today_date_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    // Hinnant's civil_from_days — days since 1970-01-01 (UTC) → (y, m, d)
    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}_{m:02}_{d:02}")
}

fn write_last_workspace_path(app: &AppHandle, workspace_path: &str) -> CommandResult<()> {
    let path = last_workspace_path_file(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ErrorDto::io(parent, &error))?;
    }
    fs::write(&path, workspace_path).map_err(|error| ErrorDto::io(&path, &error))
}

fn clear_persisted_last_workspace_path(app: &AppHandle) -> CommandResult<bool> {
    let path = last_workspace_path_file(app)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ErrorDto::io(&path, &error)),
    }
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

    fn page_summary(id: &[&str], parent: Option<&[&str]>) -> PageSummary {
        let page_id = PageId::new(id.iter().copied()).unwrap();
        PageSummary {
            page_id,
            location: PageLocation::Pages,
            workspace_path: PathBuf::new(),
            title: id.last().copied().unwrap_or_default().to_owned(),
            revision: FileFingerprint::from_text(""),
            modified_at: None,
            parent_page_id: parent.map(|segments| PageId::new(segments.iter().copied()).unwrap()),
            child_page_count: 0,
        }
    }

    fn open_test_session(root: &Path) -> WorkspaceSession {
        fs::create_dir_all(root.join("pages")).unwrap();
        WorkspaceSession::open(root).unwrap()
    }

    #[test]
    fn workspace_storage_paths_share_the_same_root_directory() {
        let app_data_dir = PathBuf::from("/data/user/0/com.seildancer.uniseq");
        let storage_root = workspace_storage_dir_from_app_data(&app_data_dir);

        assert_eq!(storage_root, app_data_dir.join("workspace"));
        assert_eq!(
            default_workspace_path_from_storage_root(&storage_root),
            app_data_dir.join("workspace").join("default_workspace")
        );
        assert_eq!(
            storage_root.join(safe_remote_folder_name(
                "https://sync.example.com/workspace-123"
            )),
            app_data_dir
                .join("workspace")
                .join("https-sync.example.com-workspace-123")
        );
    }

    #[test]
    fn page_id_string_round_trips_for_pages_and_streams() {
        let page = PageId::new(["A", "B"]).unwrap();
        let stream = PageId::stream(
            PageName::new("journal").unwrap(),
            PageName::new("2026_05_07").unwrap(),
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
        assert_eq!(
            parse_page_id_input("streams/journal/2026_05_07").unwrap(),
            stream
        );
        assert!(parse_page_id_input("streams/journal").is_err());
    }

    #[test]
    fn file_fingerprint_json_carries_u64_hash_as_string() {
        let fingerprint = FileFingerprint::from_parts(3, u64::MAX);
        let value = serde_json::to_value(FileFingerprintDto::from(fingerprint)).unwrap();

        assert_eq!(
            value["content_hash"],
            serde_json::Value::String(u64::MAX.to_string())
        );

        let input: FileFingerprintInputDto = serde_json::from_value(value).unwrap();
        assert_eq!(FileFingerprint::from(input), fingerprint);
    }

    #[test]
    fn read_commands_fail_cleanly_without_an_open_workspace() {
        let controller = WorkspaceController::default();

        let error = controller.all_pages().unwrap_err();
        assert_eq!(error.code, "no_workspace_open");
    }

    #[test]
    fn open_workspace_reads_pages_and_exposes_watcher_status() {
        let root = unique_temp_dir("uniseq-app-open");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/A.md", "- [[B]]\n");
        write_workspace_file(&root, "pages/B.md", "");

        let mut controller = WorkspaceController::default();
        let opened = controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(opened.root_path, root.to_string_lossy());
        assert!(root.join("pages").is_dir());
        assert!(root.join("assets").is_dir());
        assert!(root.join("uniseq").is_dir());
        assert!(root.join("journals").is_dir());
        assert!(root.join("diary").is_dir());
        let pages = controller.all_pages().unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_id, "pages:A");

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn search_pages_returns_ranked_results_from_open_workspace() {
        let root = unique_temp_dir("uniseq-app-search");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/Alpha.md", "misc");
        write_workspace_file(&root, "pages/Beta.md", "alpha in content");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        let results = controller
            .search_pages("alpha".to_owned(), Some(10))
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].page_id, "pages:Alpha");
        assert_eq!(results[0].matched_field, "title");
        assert_eq!(results[1].page_id, "pages:Beta");
        assert_eq!(results[1].matched_field, "content");
        assert!(
            results[1]
                .snippet
                .as_ref()
                .is_some_and(|snippet| snippet.contains("alpha"))
        );

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_events_and_watch_errors_are_surfaceable() {
        let root = unique_temp_dir("uniseq-app-events");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/A.md", "- [[B]]\n");
        write_workspace_file(&root, "pages/B.md", "");
        write_workspace_file(&root, "pages/C.md", "");

        let mut controller = WorkspaceController {
            session: Some(WorkspaceSession::open(&root).unwrap()),
            sync_loop: None,
            active_ai_chat_session: None,
            next_ai_chat_session_nonce: 0,
        };
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

    #[test]
    fn create_workspace_bootstraps_pages_directory_and_opens_session() {
        let root = unique_temp_dir("uniseq-app-create");
        fs::create_dir_all(&root).unwrap();

        let mut controller = WorkspaceController::default();
        let opened = controller
            .create_workspace(root.to_string_lossy().to_string(), "Notebook".to_owned())
            .unwrap();

        let workspace_root = root.join("Notebook");
        assert_eq!(PathBuf::from(&opened.root_path), workspace_root);
        assert!(workspace_root.join("pages").is_dir());
        assert!(workspace_root.join("assets").is_dir());
        assert!(workspace_root.join("uniseq").is_dir());
        assert!(workspace_root.join("journals").is_dir());
        assert!(workspace_root.join("diary").is_dir());
        let today = today_date_name();
        let page_ids = controller
            .all_pages()
            .unwrap()
            .into_iter()
            .map(|page| page.page_id)
            .collect::<Vec<_>>();
        assert_eq!(page_ids.len(), 4);
        assert!(page_ids.contains(&format!("stream:journals/{today}")));
        assert!(page_ids.contains(&format!("stream:diary/{today}")));
        assert!(page_ids.contains(&"pages:page".to_owned()));
        assert!(page_ids.contains(&"pages:page/subpage".to_owned()));

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_workspace_reopens_existing_workspace_target() {
        let root = unique_temp_dir("uniseq-app-create-existing");
        fs::create_dir_all(root.join("Notebook").join("pages")).unwrap();
        write_workspace_file(&root.join("Notebook"), "pages/A.md", "");

        let mut controller = WorkspaceController::default();
        let opened = controller
            .create_workspace(root.to_string_lossy().to_string(), "Notebook".to_owned())
            .unwrap();

        assert_eq!(PathBuf::from(&opened.root_path), root.join("Notebook"));
        assert!(root.join("Notebook").join("journals").is_dir());
        assert!(root.join("Notebook").join("diary").is_dir());
        let today = today_date_name();
        let page_ids = controller
            .all_pages()
            .unwrap()
            .into_iter()
            .map(|page| page.page_id)
            .collect::<Vec<_>>();
        assert_eq!(page_ids.len(), 5);
        assert!(page_ids.contains(&"pages:A".to_owned()));
        assert!(page_ids.contains(&format!("stream:journals/{today}")));
        assert!(page_ids.contains(&format!("stream:diary/{today}")));
        assert!(page_ids.contains(&"pages:page".to_owned()));
        assert!(page_ids.contains(&"pages:page/subpage".to_owned()));

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rename_stream_reopens_workspace_with_updated_stream_pages() {
        let root = unique_temp_dir("uniseq-app-rename-stream");
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join("scratch")).unwrap();
        write_workspace_file(&root, "scratch/2026_05_14.md", "hello");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        controller
            .rename_stream("scratch".to_owned(), "ideas".to_owned())
            .unwrap();

        assert!(root.join("ideas").join("2026_05_14.md").is_file());
        assert!(!root.join("scratch").exists());
        assert_eq!(
            controller.all_streams().unwrap(),
            vec![
                "diary".to_owned(),
                "ideas".to_owned(),
                "journals".to_owned()
            ]
        );
        assert!(
            controller
                .all_pages()
                .unwrap()
                .iter()
                .any(|page| page.page_id == "stream:ideas/2026_05_14")
        );

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_workspace_folder_names_are_rejected() {
        let root = unique_temp_dir("uniseq-app-invalid-name");
        fs::create_dir_all(&root).unwrap();

        let mut controller = WorkspaceController::default();
        let error = controller
            .create_workspace(root.to_string_lossy().to_string(), "  ".to_owned())
            .unwrap_err();
        assert_eq!(error.code, "invalid_workspace_name");

        let error = controller
            .create_workspace(root.to_string_lossy().to_string(), "bad/name".to_owned())
            .unwrap_err();
        assert_eq!(error.code, "invalid_workspace_name");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn open_workspace_rejects_default_stream_file_collisions() {
        let root = unique_temp_dir("uniseq-app-open-invalid-stream-default");
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::write(root.join("journals"), "").unwrap();

        let mut controller = WorkspaceController::default();
        let error = controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap_err();

        assert_eq!(error.code, "invalid_workspace_structure");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn open_workspace_backfills_assets_and_uniseq() {
        let root = unique_temp_dir("uniseq-app-open-backfill");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/A.md", "");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        assert!(root.join("assets").is_dir());
        assert!(root.join("uniseq").is_dir());
        assert!(root.join("journals").is_dir());
        assert!(root.join("diary").is_dir());

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_page_content_creates_missing_stream_page_on_demand() {
        let root = unique_temp_dir("uniseq-app-stream-write-on-demand");
        fs::create_dir_all(root.join("pages")).unwrap();

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        let written = controller
            .write_page_content(
                "stream:diary/2026_05_14".to_owned(),
                "- first line\n".to_owned(),
                None,
            )
            .unwrap();

        assert_eq!(written.text, "- first line\n");
        assert_eq!(
            fs::read_to_string(root.join("diary").join("2026_05_14.md")).unwrap(),
            "- first line\n"
        );

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_empty_stream_pages_removes_only_old_empty_stream_files() {
        let root = unique_temp_dir("uniseq-app-stream-cleanup");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "diary/2020_01_01.md", "   \n");
        write_workspace_file(&root, "journals/2020_01_01.md", "- keep\n");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        let result = controller.cleanup_empty_stream_pages(7).unwrap();
        assert_eq!(
            result.removed_page_ids,
            vec!["stream:diary/2020_01_01".to_owned()]
        );
        assert!(!root.join("diary").join("2020_01_01.md").exists());
        assert_eq!(
            fs::read_to_string(root.join("journals").join("2020_01_01.md")).unwrap(),
            "- keep\n"
        );

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn find_empty_pages_lists_empty_stream_pages_and_leaf_pages_only() {
        let root = unique_temp_dir("uniseq-app-find-empty-pages");
        fs::create_dir_all(root.join("pages")).unwrap();
        write_workspace_file(&root, "pages/EmptyLeaf.md", " \n");
        write_workspace_file(&root, "pages/Parent.md", "");
        write_workspace_file(&root, "pages/Parent___Child.md", "- keep\n");
        write_workspace_file(&root, "pages/NonEmpty.md", "text\n");
        write_workspace_file(&root, "pages/RefTarget.md", " \n");
        write_workspace_file(&root, "pages/RefSource.md", "- [[RefTarget]]\n");
        write_workspace_file(&root, "diary/2020_01_01.md", "\n");
        write_workspace_file(&root, "journals/2020_01_01.md", "- keep\n");

        let mut controller = WorkspaceController::default();
        controller
            .open_workspace(root.to_string_lossy().to_string())
            .unwrap();

        let empty_page_ids = controller
            .find_empty_pages()
            .unwrap()
            .into_iter()
            .map(|page| page.page_id)
            .collect::<Vec<_>>();

        assert_eq!(
            empty_page_ids,
            vec![
                "pages:EmptyLeaf".to_owned(),
                "stream:diary/2020_01_01".to_owned()
            ]
        );

        controller.close_workspace();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn normalize_workspace_page_order_keeps_manual_order_and_appends_new_siblings() {
        let workspace_order = WorkspacePageOrder {
            sibling_order_by_parent: BTreeMap::from([(
                ROOT_PARENT_ORDER_KEY.to_owned(),
                vec!["pages:B".to_owned(), "pages:A".to_owned()],
            )]),
        };
        let normalized = normalize_workspace_page_order(
            &workspace_order,
            &[
                page_summary(&["A"], None),
                page_summary(&["B"], None),
                page_summary(&["C"], None),
            ],
        );

        assert_eq!(
            normalized.sibling_order_by_parent[ROOT_PARENT_ORDER_KEY],
            vec![
                "pages:B".to_owned(),
                "pages:A".to_owned(),
                "pages:C".to_owned(),
            ]
        );
    }

    #[test]
    fn remap_workspace_page_order_subtree_updates_parent_keys_and_values() {
        let mut workspace_order = WorkspacePageOrder {
            sibling_order_by_parent: BTreeMap::from([
                (
                    ROOT_PARENT_ORDER_KEY.to_owned(),
                    vec!["pages:A".to_owned(), "pages:X".to_owned()],
                ),
                (
                    "pages:A".to_owned(),
                    vec!["pages:A/B".to_owned(), "pages:A/C".to_owned()],
                ),
                ("pages:A/B".to_owned(), vec!["pages:A/B/D".to_owned()]),
            ]),
        };

        remap_workspace_page_order_subtree(&mut workspace_order, "pages:A/B", "pages:Z/B");

        assert_eq!(
            workspace_order.sibling_order_by_parent["pages:A"],
            vec!["pages:Z/B".to_owned(), "pages:A/C".to_owned()]
        );
        assert_eq!(
            workspace_order.sibling_order_by_parent["pages:Z/B"],
            vec!["pages:Z/B/D".to_owned()]
        );
    }

    #[test]
    fn remove_workspace_page_order_subtree_drops_deleted_subtree_keys_and_values() {
        let mut workspace_order = WorkspacePageOrder {
            sibling_order_by_parent: BTreeMap::from([
                (
                    ROOT_PARENT_ORDER_KEY.to_owned(),
                    vec!["pages:A".to_owned(), "pages:X".to_owned()],
                ),
                (
                    "pages:A".to_owned(),
                    vec!["pages:A/B".to_owned(), "pages:A/C".to_owned()],
                ),
                ("pages:A/B".to_owned(), vec!["pages:A/B/D".to_owned()]),
            ]),
        };

        remove_workspace_page_order_subtree(&mut workspace_order, "pages:A/B");

        assert_eq!(
            workspace_order.sibling_order_by_parent["pages:A"],
            vec!["pages:A/C".to_owned()]
        );
        assert!(
            !workspace_order
                .sibling_order_by_parent
                .contains_key("pages:A/B")
        );
    }

    #[test]
    fn ai_page_session_includes_page_body_and_linked_refs() {
        let root = unique_temp_dir("uniseq-app-ai-page");
        write_workspace_file(&root, "pages/Target.md", "main body");
        write_workspace_file(&root, "pages/Source.md", "- [[Target]]\n  linked thought");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::Page {
                page_id: "pages:Target".to_owned(),
            },
            2_000,
        )
        .unwrap();

        assert!(prepared.system_prompt.contains("main body"));
        assert!(prepared.system_prompt.contains("linked thought"));
        assert!(prepared.preview_summary.contains("Target"));
        assert!(prepared.preview_summary.contains("Source"));
        assert!(prepared
            .system_prompt
            .contains("time-agnostic reference information"));
        assert_eq!(
            prepared.included_page_ids,
            vec!["pages:Target".to_owned(), "pages:Source".to_owned()]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_page_session_labels_stream_backed_related_notes_with_stream_name() {
        let root = unique_temp_dir("uniseq-app-ai-page-stream-ref");
        write_workspace_file(&root, "pages/Target.md", "main body");
        write_workspace_file(&root, "journals/2026_05_20.md", "- [[Target]]\n  linked thought");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::Page {
                page_id: "pages:Target".to_owned(),
            },
            2_000,
        )
        .unwrap();

        assert!(prepared.system_prompt.contains("Related note: 2026_05_20 (journals)"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_page_preview_summarizes_stream_related_notes_as_date_ranges() {
        let root = unique_temp_dir("uniseq-app-ai-page-stream-range");
        write_workspace_file(&root, "pages/Target.md", "main body");
        write_workspace_file(&root, "journals/2026_05_18.md", "- [[Target]]\n  first linked thought");
        write_workspace_file(&root, "journals/2026_05_20.md", "- [[Target]]\n  second linked thought");
        write_workspace_file(&root, "pages/Project.md", "- [[Target]]\n  page linked thought");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::Page {
                page_id: "pages:Target".to_owned(),
            },
            4_000,
        )
        .unwrap();

        assert!(prepared.preview_summary.contains("Project"));
        assert!(prepared
            .preview_summary
            .contains("journals from 2026-05-18 to 2026-05-20"));
        assert!(!prepared.preview_summary.contains("2026_05_18 (journals)"));
        assert!(!prepared.preview_summary.contains("2026_05_20 (journals)"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_single_stream_session_orders_notes_newest_to_oldest() {
        let root = unique_temp_dir("uniseq-app-ai-single-stream");
        write_workspace_file(&root, "journals/2026_05_19.md", "older note");
        write_workspace_file(&root, "journals/2026_05_20.md", "newer note");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::StreamSingle {
                stream_name: "journals".to_owned(),
            },
            4_000,
        )
        .unwrap();

        let newer_index = prepared
            .system_prompt
            .find("2026-05-20\njournals")
            .unwrap();
        let older_index = prepared
            .system_prompt
            .find("2026-05-19\njournals")
            .unwrap();
        assert!(newer_index < older_index);
        assert!(!prepared
            .system_prompt
            .contains("time-agnostic reference information"));
        assert_eq!(
            prepared.included_page_ids,
            vec![
                "stream:journals/2026_05_20".to_owned(),
                "stream:journals/2026_05_19".to_owned()
            ]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_dual_stream_session_interleaves_by_date_and_stream_order() {
        let root = unique_temp_dir("uniseq-app-ai-dual-stream");
        write_workspace_file(&root, "journals/2026_05_20.md", "journal latest");
        write_workspace_file(&root, "diary/2026_05_20.md", "diary latest");
        write_workspace_file(&root, "journals/2026_05_19.md", "journal older");
        write_workspace_file(&root, "diary/2026_05_18.md", "diary oldest");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::StreamDual {
                stream_names: vec!["journals".to_owned(), "diary".to_owned()],
            },
            8_000,
        )
        .unwrap();

        assert_eq!(
            prepared.included_page_ids,
            vec![
                "stream:journals/2026_05_20".to_owned(),
                "stream:diary/2026_05_20".to_owned(),
                "stream:journals/2026_05_19".to_owned(),
                "stream:diary/2026_05_18".to_owned()
            ]
        );
        assert_eq!(
            prepared.preview_summary,
            "Start chat based on journals and diary from 2026-05-18 to 2026-05-20."
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_stream_truncation_drops_oldest_notes_first_and_updates_preview_range() {
        let root = unique_temp_dir("uniseq-app-ai-stream-truncation");
        write_workspace_file(&root, "journals/2026_05_18.md", "old note old note old note old note old note old note old note old note");
        write_workspace_file(&root, "journals/2026_05_19.md", "mid note mid note mid note mid note mid note mid note mid note mid note");
        write_workspace_file(&root, "journals/2026_05_20.md", "new note new note new note new note new note new note new note new note");
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::StreamSingle {
                stream_name: "journals".to_owned(),
            },
            260,
        )
        .unwrap();

        assert!(prepared.truncated);
        assert_eq!(
            prepared.included_page_ids,
            vec![
                "stream:journals/2026_05_20".to_owned(),
                "stream:journals/2026_05_19".to_owned()
            ]
        );
        assert_eq!(
            prepared.preview_summary,
            "Start chat based on journals from 2026-05-19 to 2026-05-20."
        );
        assert!(!prepared.system_prompt.contains("2026-05-18\njournals"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ai_page_truncation_preserves_page_body_before_linked_refs() {
        let root = unique_temp_dir("uniseq-app-ai-page-truncation");
        write_workspace_file(
            &root,
            "pages/Target.md",
            "page body page body page body page body page body",
        );
        write_workspace_file(
            &root,
            "pages/Source.md",
            "- [[Target]]\n  ref body ref body ref body ref body ref body",
        );
        let session = open_test_session(&root);

        let prepared = build_ai_chat_session(
            &session,
            &AiChatContextSpecDto::Page {
                page_id: "pages:Target".to_owned(),
            },
            90,
        )
        .unwrap();

        assert!(prepared.truncated);
        assert!(prepared.system_prompt.contains("page body"));
        assert!(!prepared.system_prompt.contains("ref body"));
        assert_eq!(prepared.included_page_ids, vec!["pages:Target".to_owned()]);

        let _ = fs::remove_dir_all(root);
    }
}

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uniseq_engine::{
    append_journal_entry, build_index, clear_snapshot, create_workspace, edit_markdown_span,
    graph_data, load_or_rebuild, load_settings, load_snapshot, move_asset, open_workspace,
    query_assets, query_journal, query_page, query_timeline, rename_page, save_settings,
    save_snapshot, scan_assets, scan_compatibility, scan_plugins, search, search_with_options,
    sync_now, sync_plan, task_query, task_rollup, toggle_task, update_page_front_matter,
    update_surface_statuses, AppSettings, AssetQuery, AssetRecord, AssetRegistry,
    FeatureRegistry, GraphData, IndexSnapshot, PageProjection, PluginRegistry, SearchHit,
    SearchOptions, SearchResult, SourceAnchor, SyncState, TaskQuery, TaskState, TimelineEntry,
    WorkspaceIndex, WorkspaceSummary, WorkspaceWarning, WriteResult,
};

/// Global workspace state managed by Rust — tracks the canonical root of the
/// currently open workspace for path-trust enforcement.
static ACTIVE_WORKSPACE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set active workspace root (called after open/create succeeds).
fn set_active_workspace(root: PathBuf) {
    let mut guard = ACTIVE_WORKSPACE.lock().unwrap();
    *guard = Some(root);
}

/// Clear active workspace root (call on close or error).
fn clear_active_workspace() {
    let mut guard = ACTIVE_WORKSPACE.lock().unwrap();
    *guard = None;
}

/// Wrapper for commands that need workspace path trust.
/// Takes path arg, canonicalizes it, checks against active root, returns canonical root.
fn require_workspace_root(path: &str) -> Result<PathBuf, String> {
    let canonical = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    let guard = ACTIVE_WORKSPACE.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(active) if active != &canonical => {
            Err(format!(
                "path mismatch: active workspace is {:?}, received {:?}. Use active root.",
                active, canonical
            ))
        }
        Some(_) => Ok(canonical),
        None => Err("no active workspace; open or create a workspace first".to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AppSnapshot {
    workspace: WorkspaceSummary,
    index: WorkspaceIndex,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppSnapshotWithInvalidations {
    workspace: WorkspaceSummary,
    index: WorkspaceIndex,
    invalidated: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WriteResultDto {
    anchor: Option<SourceAnchor>,
    invalidated: Vec<PathBuf>,
}

impl From<WriteResult> for WriteResultDto {
    fn from(r: WriteResult) -> Self {
        WriteResultDto { anchor: r.anchor, invalidated: r.invalidated }
    }
}

#[tauri::command]
fn create_workspace_cmd(path: String) -> Result<WorkspaceSummary, String> {
    clear_active_workspace();
    let result = create_workspace(&path).map_err(|e| e.to_string())?;
    let canonical = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    set_active_workspace(canonical);
    Ok(result)
}

#[tauri::command]
fn open_workspace_cmd(path: String) -> Result<AppSnapshot, String> {
    clear_active_workspace();
    let canonical = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    let workspace = open_workspace(&canonical).map_err(|e| e.to_string())?;
    let index = build_index(&canonical).map_err(|e| e.to_string())?;
    set_active_workspace(canonical);
    Ok(AppSnapshot { workspace, index })
}

#[tauri::command]
fn open_workspace_with_cache_cmd(path: String) -> Result<AppSnapshotWithInvalidations, String> {
    clear_active_workspace();
    let canonical = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    let workspace = open_workspace(&canonical).map_err(|e| e.to_string())?;
    let (index, _) = load_or_rebuild(&canonical).map_err(|e| e.to_string())?;
    set_active_workspace(canonical);
    Ok(AppSnapshotWithInvalidations { workspace, index, invalidated: vec![] })
}

#[tauri::command]
fn query_journal_cmd(path: String, date: String) -> Result<Vec<uniseq_engine::Entry>, String> {
    let root = require_workspace_root(&path)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(query_journal(&index, date))
}

#[tauri::command]
fn query_timeline_cmd(path: String) -> Result<Vec<TimelineEntry>, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(query_timeline(&index))
}

#[tauri::command]
fn query_page_cmd(path: String, page: String) -> Result<PageProjection, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(query_page(&index, &page))
}

#[tauri::command]
fn search_cmd(path: String, query: String) -> Result<Vec<SearchHit>, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(search(&index, &query))
}

#[tauri::command]
fn structured_search_cmd(path: String, options: SearchOptions) -> Result<SearchResult, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(search_with_options(&index, &options))
}

#[tauri::command]
fn tasks_cmd(path: String) -> Result<Vec<uniseq_engine::Entry>, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(task_rollup(&index))
}

#[tauri::command]
fn task_query_cmd(path: String, query: TaskQuery) -> Result<Vec<uniseq_engine::Entry>, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(task_query(&index, query))
}

#[tauri::command]
fn graph_data_cmd(path: String) -> Result<GraphData, String> {
    let root = require_workspace_root(&path)?;
    let index = build_index(root).map_err(|e| e.to_string())?;
    Ok(graph_data(&index))
}

#[tauri::command]
fn asset_registry_cmd(path: String) -> Result<AssetRegistry, String> {
    let root = require_workspace_root(&path)?;
    Ok(scan_assets(root, &[]))
}

#[tauri::command]
fn query_assets_cmd(path: String, query: AssetQuery) -> Result<Vec<AssetRecord>, String> {
    let root = require_workspace_root(&path)?;
    let registry = scan_assets(root, &[]);
    Ok(query_assets(&registry, &query))
}

#[tauri::command]
fn feature_surfaces_cmd(path: String) -> Result<FeatureRegistry, String> {
    let root = require_workspace_root(&path)?;
    Ok(update_surface_statuses(&root))
}

#[tauri::command]
fn plugins_cmd(path: String) -> Result<PluginRegistry, String> {
    let root = require_workspace_root(&path)?;
    scan_plugins(root).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_settings_cmd(path: String) -> Result<AppSettings, String> {
    let root = require_workspace_root(&path)?;
    load_settings(root).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings_cmd(path: String, settings: AppSettings) -> Result<(), String> {
    let root = require_workspace_root(&path)?;
    save_settings(root, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_plan_cmd(path: String) -> Result<SyncState, String> {
    let root = require_workspace_root(&path)?;
    sync_plan(root).map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_now_cmd(path: String) -> Result<SyncState, String> {
    let root = require_workspace_root(&path)?;
    sync_now(root).map_err(|e| e.to_string())
}

#[tauri::command]
fn append_journal_entry_cmd(path: String, date: String, markdown: String) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    append_journal_entry(root, date, &markdown)
        .map_err(|e| e.to_string())
        .map(WriteResultDto::from)
}

#[tauri::command]
fn toggle_task_cmd(path: String, anchor: SourceAnchor, desired: Option<TaskState>) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    let anchor = sanitize_anchor(&root, anchor)?;
    toggle_task(&anchor, desired).map_err(|e| e.to_string()).map(WriteResultDto::from)
}

#[tauri::command]
fn rename_page_cmd(path: String, from: String, to: String) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    rename_page(root, &from, &to).map_err(|e| e.to_string()).map(WriteResultDto::from)
}

#[tauri::command]
fn edit_markdown_span_cmd(path: String, anchor: SourceAnchor, replacement: String) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    let anchor = sanitize_anchor(&root, anchor)?;
    edit_markdown_span(&anchor, &replacement).map_err(|e| e.to_string()).map(WriteResultDto::from)
}

#[tauri::command]
fn update_page_front_matter_cmd(path: String, page: String, front_matter_body: String) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    update_page_front_matter(root, &page, &front_matter_body).map_err(|e| e.to_string()).map(WriteResultDto::from)
}

#[tauri::command]
fn move_asset_cmd(path: String, from_relative: String, to_relative: String) -> Result<WriteResultDto, String> {
    let root = require_workspace_root(&path)?;
    move_asset(root, &from_relative, &to_relative).map_err(|e| e.to_string()).map(WriteResultDto::from)
}

#[tauri::command]
fn compatibility_scan_cmd(path: String) -> Result<Vec<WorkspaceWarning>, String> {
    let root = require_workspace_root(&path)?;
    Ok(scan_compatibility(root))
}

#[tauri::command]
fn save_cache_snapshot_cmd(path: String) -> Result<(), String> {
    let root = require_workspace_root(&path)?;
    let workspace = open_workspace(&root).map_err(|e| e.to_string())?;
    let index = build_index(&root).map_err(|e| e.to_string())?;
    save_snapshot(&root, index, workspace.warnings).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_cache_snapshot_cmd(path: String) -> Result<(), String> {
    let root = require_workspace_root(&path)?;
    clear_snapshot(root).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_cache_snapshot_cmd(path: String) -> Result<Option<IndexSnapshot>, String> {
    let root = require_workspace_root(&path)?;
    load_snapshot(root).map_err(|e| e.to_string())
}

fn sanitize_anchor(root: &PathBuf, mut anchor: SourceAnchor) -> Result<SourceAnchor, String> {
    let file = PathBuf::from(&anchor.file_path).canonicalize().map_err(|e| e.to_string())?;
    let journals = root.join("journals");
    let pages = root.join("pages");
    if !file.starts_with(&journals) && !file.starts_with(&pages) {
        return Err("source anchor must point to a journal or page inside the workspace".to_string());
    }
    anchor.file_path = file;
    Ok(anchor)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            create_workspace_cmd,
            open_workspace_cmd,
            open_workspace_with_cache_cmd,
            query_journal_cmd,
            query_timeline_cmd,
            query_page_cmd,
            search_cmd,
            structured_search_cmd,
            tasks_cmd,
            task_query_cmd,
            graph_data_cmd,
            asset_registry_cmd,
            query_assets_cmd,
            feature_surfaces_cmd,
            plugins_cmd,
            load_settings_cmd,
            save_settings_cmd,
            sync_plan_cmd,
            sync_now_cmd,
            append_journal_entry_cmd,
            toggle_task_cmd,
            rename_page_cmd,
            edit_markdown_span_cmd,
            update_page_front_matter_cmd,
            move_asset_cmd,
            compatibility_scan_cmd,
            save_cache_snapshot_cmd,
            clear_cache_snapshot_cmd,
            load_cache_snapshot_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Uniseq");
}

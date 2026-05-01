pub mod assets;
pub mod cache;
pub mod events;
pub mod feature_surfaces;
pub mod graph;
pub mod index;
pub mod model;
pub mod page_identity;
pub mod parser;
pub mod plugins;
pub mod query;
pub mod settings;
pub mod sync;
pub mod workspace;
pub mod writes;

// ── Cache ──────────────────────────────────────────────────────────────────────
pub use cache::{clear_snapshot, load_or_rebuild, load_snapshot, save_snapshot, IndexSnapshot};

// ── Assets ────────────────────────────────────────────────────────────────────
pub use assets::{query_assets, scan_assets, AssetQuery, AssetRecord, AssetRegistry, ReferencedAnchor};

// ── Events ─────────────────────────────────────────────────────────────────────
pub use events::{
    file_changed_events, page_invalidated_from_invalidations, workspace_opened_event,
    ConflictResolution, EngineEvent, EngineEventKind, FileChangeKind,
};

// ── Feature surfaces ───────────────────────────────────────────────────────────
pub use feature_surfaces::{update_surface_statuses, FeatureRegistry, FeatureStatus, FeatureSurface};

// ── Graph ──────────────────────────────────────────────────────────────────────
pub use graph::{graph_data, GraphData, GraphEdge, GraphNode, NodeKind};

// ── Index ─────────────────────────────────────────────────────────────────────
pub use index::{
    build_index, query_journal, query_page, query_timeline, search, task_rollup, TimelineEntry,
    WorkspaceIndex,
};

// ── Model ─────────────────────────────────────────────────────────────────────
pub use model::*;

// ── Page identity ─────────────────────────────────────────────────────────────
pub use page_identity::{filename_to_page_path, normalize_page_path, page_path_to_filename};

// ── Parser ─────────────────────────────────────────────────────────────────────
pub use parser::parse_markdown_file;

// ── Plugins ────────────────────────────────────────────────────────────────────
pub use plugins::{has_capability, scan_plugins, Capability, PluginError, PluginManifest, PluginRegistry};

// ── Query ─────────────────────────────────────────────────────────────────────
pub use query::{search_with_options, search_with_options as structured_search, task_query, SearchOptions, SearchResult, TaskQuery, TaskStateFilter};

// ── Settings ──────────────────────────────────────────────────────────────────
pub use settings::{load_settings, save_settings, AppSettings, CalendarSettings, EditorSettings, PluginSettings, SettingsError, SyncSettings, ThemeSettings};

// ── Sync ─────────────────────────────────────────────────────────────────────
pub use sync::{build_manifest, load_manifest, save_manifest, sync_now, sync_plan, ConflictEntry, SyncError, SyncManifest, SyncManifestEntry, SyncState};

// ── Workspace ─────────────────────────────────────────────────────────────────
pub use workspace::{
    create_workspace, detect_degraded_logseq_constructs_on_disk, open_workspace, validate_workspace,
};

// ── Writes ─────────────────────────────────────────────────────────────────────
pub use writes::{
    append_journal_entry, edit_markdown_span, move_asset, read_task_state, rename_page,
    toggle_task, update_page_front_matter, WriteError, WriteResult,
};

// ── Compatibility scan (free function) ────────────────────────────────────────
/// Returns warnings for unsupported/degraded Logseq constructs
/// without requiring a full workspace open or index build.
pub fn scan_compatibility(root: impl AsRef<std::path::Path>) -> Vec<crate::model::WorkspaceWarning> {
    let mut warnings = Vec::new();
    crate::workspace::detect_degraded_logseq_constructs_on_disk(root, &mut warnings);
    warnings
}
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub root: PathBuf,
    pub journals_dir: PathBuf,
    pub pages_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub whiteboards_dir: PathBuf,
    pub pdf_dir: PathBuf,
    pub app_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub config: WorkspaceConfig,
    pub journals: Vec<JournalFile>,
    pub pages: Vec<PageFile>,
    pub warnings: Vec<WorkspaceWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub schema_version: u32,
    pub app_version: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalFile {
    pub date: NaiveDate,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageFile {
    pub page_path: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceWarning {
    pub path: Option<PathBuf>,
    pub kind: WarningKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WarningKind {
    MissingDirectory,
    InvalidJournalName,
    InvalidPageName,
    UnsupportedLogseqConstruct,
    InvalidConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentKind {
    Journal { date: NaiveDate },
    Page { page_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedDocument {
    pub kind: DocumentKind,
    pub path: PathBuf,
    pub front_matter: Option<String>,
    pub aliases: Vec<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// Ephemeral runtime identifier. It is not written back to markdown.
    pub runtime_id: String,
    pub text: String,
    pub level: EntryLevel,
    pub task: Option<TaskState>,
    pub links: Vec<PageRef>,
    pub tags: Vec<PageRef>,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryLevel {
    Heading(u8),
    ListItem,
    Paragraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Todo,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageRef {
    pub raw: String,
    pub page_path: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SourceAnchor {
    pub file_path: PathBuf,
    pub span: Span,
    pub snippet: String,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageProjection {
    pub page_path: String,
    pub own_entries: Vec<Entry>,
    pub incoming_entries: Vec<Entry>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchHit {
    pub entry: Entry,
    pub score: usize,
}

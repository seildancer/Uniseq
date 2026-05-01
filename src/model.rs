use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JournalDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl JournalDate {
    pub fn parse(input: &str) -> Option<Self> {
        let mut parts = input.split('-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn from_journal_file_name(file_name: &str) -> Option<Self> {
        file_name
            .strip_suffix(".md")
            .and_then(Self::parse)
    }
}

impl fmt::Display for JournalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageKey(String);

impl PageKey {
    pub fn new(raw: impl AsRef<str>) -> Option<Self> {
        let normalized = normalize_page_key(raw.as_ref());
        if normalized.is_empty() {
            None
        } else {
            Some(Self(normalized))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn namespace_segments(&self) -> Vec<&str> {
        self.0.split('/').collect()
    }
}

impl fmt::Display for PageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentKind {
    Journal(JournalDate),
    Page(PageKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Paragraph,
    Task { checked: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAnchor {
    pub file_path: PathBuf,
    pub span: Span,
    pub snippet: String,
    pub snippet_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    Tag,
    PageLink,
    Asset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineReference {
    pub kind: ReferenceKind,
    pub raw: String,
    pub page_key: Option<PageKey>,
    pub path: Option<String>,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub text: String,
    pub checked: bool,
    pub linked_pages: Vec<PageKey>,
    pub anchor: SourceAnchor,
    pub marker_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub text: String,
    pub anchor: SourceAnchor,
    pub references: Vec<InlineReference>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontMatterValue {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatter {
    pub values: BTreeMap<String, FrontMatterValue>,
}

impl FrontMatter {
    pub fn aliases(&self) -> Vec<String> {
        match self.values.get("aliases") {
            Some(FrontMatterValue::List(items)) => items.clone(),
            Some(FrontMatterValue::Scalar(value)) => vec![value.clone()],
            None => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDocument {
    pub kind: DocumentKind,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub front_matter: FrontMatter,
    pub body: String,
    pub entries: Vec<Entry>,
    pub issues: Vec<CompatibilityIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityIssue {
    pub severity: CompatibilitySeverity,
    pub relative_path: PathBuf,
    pub construct: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilitySeverity {
    Supported,
    Adapted,
    Degraded,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub workspace_version: u32,
    pub migration_placeholder: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            workspace_version: 1,
            migration_placeholder: "reserved".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub journals: PathBuf,
    pub pages: PathBuf,
    pub assets: PathBuf,
    pub whiteboards: PathBuf,
    pub pdf: PathBuf,
    pub app: PathBuf,
    pub cache: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub root: PathBuf,
    pub journal_files: usize,
    pub page_files: usize,
    pub asset_files: usize,
    pub supported_paths: Vec<PathBuf>,
    pub issues: Vec<CompatibilityIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRecord {
    pub key: PageKey,
    pub display_title: String,
    pub aliases: HashSet<String>,
    pub has_page_file: bool,
    pub page_file: Option<PathBuf>,
    pub namespaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub target: PageKey,
    pub source_anchor: SourceAnchor,
    pub source_path: PathBuf,
    pub source_date: Option<JournalDate>,
    pub source_page: Option<PageKey>,
    pub source_text: String,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: PathBuf,
    pub anchor: SourceAnchor,
    pub excerpt: String,
    pub source_date: Option<JournalDate>,
    pub linked_pages: Vec<PageKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingReference {
    pub edge: Edge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageView {
    pub page: PageRecord,
    pub page_body: Option<String>,
    pub incoming: Vec<IncomingReference>,
    pub open_tasks: Vec<Task>,
    pub related_pages: Vec<PageKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    pub anchor: SourceAnchor,
    pub source_date: Option<JournalDate>,
    pub source_page: Option<PageKey>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatterPatch {
    pub values: BTreeMap<String, FrontMatterValue>,
}

pub fn normalize_page_key(raw: &str) -> String {
    raw.split('/')
        .filter_map(|segment| {
            let segment = segment.trim();
            if segment.is_empty() {
                return None;
            }
            let mut normalized = String::new();
            let mut last_was_dash = false;
            for ch in segment.chars() {
                if ch.is_alphanumeric() {
                    normalized.extend(ch.to_lowercase());
                    last_was_dash = false;
                } else if ch == '-' || ch == '_' || ch.is_whitespace() {
                    if !last_was_dash && !normalized.is_empty() {
                        normalized.push('-');
                        last_was_dash = true;
                    }
                } else if !last_was_dash && !normalized.is_empty() {
                    normalized.push('-');
                    last_was_dash = true;
                }
            }
            let trimmed = normalized.trim_matches('-').to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub fn page_file_name_for_key(page_key: &PageKey) -> String {
    format!("{}.md", page_key.as_str().replace('/', "___"))
}

pub fn page_key_from_page_file(file_name: &str) -> Option<PageKey> {
    let stem = file_name.strip_suffix(".md")?;
    PageKey::new(stem.replace("___", "/"))
}

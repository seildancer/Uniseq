use crate::model::*;
use crate::page_identity::filename_to_page_path;
use chrono::NaiveDate;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DIRS: [&str; 7] = [
    "journals",
    "pages",
    "assets",
    "whiteboards",
    "pdf",
    "app",
    ".cache",
];

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("workspace path does not exist: {0}")]
    Missing(PathBuf),
    #[error("workspace path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(String),
}

pub fn create_workspace(root: impl AsRef<Path>) -> Result<WorkspaceSummary, WorkspaceError> {
    let root = root.as_ref();
    fs::create_dir_all(root)?;
    for dir in DIRS {
        fs::create_dir_all(root.join(dir))?;
    }
    let config_path = root.join("app").join("config.toml");
    if !config_path.exists() {
        let config = WorkspaceConfig::default();
        fs::write(
            &config_path,
            toml::to_string_pretty(&config).map_err(|e| WorkspaceError::Config(e.to_string()))?,
        )?;
    }
    open_workspace(root)
}

pub fn open_workspace(root: impl AsRef<Path>) -> Result<WorkspaceSummary, WorkspaceError> {
    let root = root.as_ref().to_path_buf();
    if !root.exists() {
        return Err(WorkspaceError::Missing(root));
    }
    if !root.is_dir() {
        return Err(WorkspaceError::NotDirectory(root));
    }

    let mut warnings = Vec::new();
    for dir in DIRS {
        if !root.join(dir).is_dir() {
            warnings.push(WorkspaceWarning {
                path: Some(root.join(dir)),
                kind: WarningKind::MissingDirectory,
                message: format!("missing canonical workspace directory `{dir}`"),
            });
        }
    }

    let config = read_config(&root, &mut warnings);
    let journals = read_journals(&root, &mut warnings)?;
    let pages = read_pages(&root, &mut warnings)?;
    detect_degraded_logseq_constructs(&journals, &pages, &mut warnings);

    Ok(WorkspaceSummary {
        journals_dir: root.join("journals"),
        pages_dir: root.join("pages"),
        assets_dir: root.join("assets"),
        whiteboards_dir: root.join("whiteboards"),
        pdf_dir: root.join("pdf"),
        app_dir: root.join("app"),
        cache_dir: root.join(".cache"),
        root,
        config,
        journals,
        pages,
        warnings,
    })
}

pub fn validate_workspace(root: impl AsRef<Path>) -> Result<Vec<WorkspaceWarning>, WorkspaceError> {
    Ok(open_workspace(root)?.warnings)
}

/// Scan for unsupported/degraded Logseq constructs across all journals and pages.
/// This does not require open_workspace and does not build an index.
pub fn detect_degraded_logseq_constructs_on_disk(
    root: impl AsRef<Path>,
    warnings: &mut Vec<WorkspaceWarning>,
) {
    let root = root.as_ref();
    let journals_dir = root.join("journals");
    let pages_dir = root.join("pages");

    let scan_dir =
        |dir: &Path, path_to_warnings: &mut FxHashMap<PathBuf, Vec<WorkspaceWarning>>| {
            if !dir.is_dir() {
                return;
            }
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    if let Ok(text) = fs::read_to_string(&path) {
                        let mut file_warnings = Vec::new();
                        for marker in [
                            "id::",
                            "collapsed::",
                            "#+BEGIN_QUERY",
                            "{{query",
                            "SCHEDULED:",
                            "DEADLINE:",
                        ] {
                            if text.contains(marker) {
                                file_warnings.push(WorkspaceWarning {
                                path: Some(path.clone()),
                                kind: WarningKind::UnsupportedLogseqConstruct,
                                message: format!("detected Logseq construct `{marker}`; it is preserved as markdown but may be degraded in Uniseq views"),
                            });
                            }
                        }
                        if !file_warnings.is_empty() {
                            path_to_warnings.insert(path, file_warnings);
                        }
                    }
                }
            }
        };

    use std::collections::HashMap;
    use std::hash::BuildHasherDefault;
    type FxHashMap<K, V> =
        HashMap<K, V, BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;
    let mut journal_warnings = FxHashMap::default();
    let mut page_warnings = FxHashMap::default();
    scan_dir(&journals_dir, &mut journal_warnings);
    scan_dir(&pages_dir, &mut page_warnings);

    for w in journal_warnings.into_values().flatten() {
        warnings.push(w);
    }
    for w in page_warnings.into_values().flatten() {
        warnings.push(w);
    }
}

fn read_config(root: &Path, warnings: &mut Vec<WorkspaceWarning>) -> WorkspaceConfig {
    let path = root.join("app").join("config.toml");
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|err| {
            warnings.push(WorkspaceWarning {
                path: Some(path),
                kind: WarningKind::InvalidConfig,
                message: err.to_string(),
            });
            WorkspaceConfig::default()
        }),
        Err(_) => WorkspaceConfig::default(),
    }
}

fn read_journals(
    root: &Path,
    warnings: &mut Vec<WorkspaceWarning>,
) -> Result<Vec<JournalFile>, WorkspaceError> {
    let dir = root.join("journals");
    let mut journals = Vec::new();
    if !dir.is_dir() {
        return Ok(journals);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
            Ok(date) => journals.push(JournalFile { date, path }),
            Err(_) => warnings.push(WorkspaceWarning {
                path: Some(path),
                kind: WarningKind::InvalidJournalName,
                message: "journal files must be named YYYY-MM-DD.md".into(),
            }),
        }
    }
    journals.sort_by_key(|j| j.date);
    Ok(journals)
}

fn read_pages(
    root: &Path,
    warnings: &mut Vec<WorkspaceWarning>,
) -> Result<Vec<PageFile>, WorkspaceError> {
    let dir = root.join("pages");
    let mut pages = Vec::new();
    if !dir.is_dir() {
        return Ok(pages);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(page_path) = filename_to_page_path(name) {
            pages.push(PageFile { page_path, path });
        } else {
            warnings.push(WorkspaceWarning {
                path: Some(path),
                kind: WarningKind::InvalidPageName,
                message: "page files must be markdown files in the flat pages directory".into(),
            });
        }
    }
    pages.sort_by(|a, b| a.page_path.cmp(&b.page_path));
    Ok(pages)
}

fn detect_degraded_logseq_constructs(
    journals: &[JournalFile],
    pages: &[PageFile],
    warnings: &mut Vec<WorkspaceWarning>,
) {
    for path in journals
        .iter()
        .map(|j| &j.path)
        .chain(pages.iter().map(|p| &p.path))
    {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for marker in [
            "id::",
            "collapsed::",
            "#+BEGIN_QUERY",
            "{{query",
            "SCHEDULED:",
            "DEADLINE:",
        ] {
            if text.contains(marker) {
                warnings.push(WorkspaceWarning {
                    path: Some(path.clone()),
                    kind: WarningKind::UnsupportedLogseqConstruct,
                    message: format!("detected Logseq construct `{marker}`; it is preserved as markdown but may be degraded in Uniseq views"),
                });
            }
        }
    }
}

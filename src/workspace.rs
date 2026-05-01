use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{
    CompatibilityIssue, CompatibilitySeverity, JournalDate, WorkspaceConfig, WorkspacePaths,
    WorkspaceSummary, page_key_from_page_file,
};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub paths: WorkspacePaths,
    pub config: WorkspaceConfig,
}

impl Workspace {
    pub fn create(root: impl AsRef<Path>) -> io::Result<Self> {
        let paths = canonical_paths(root.as_ref());
        fs::create_dir_all(&paths.journals)?;
        fs::create_dir_all(&paths.pages)?;
        fs::create_dir_all(&paths.assets)?;
        fs::create_dir_all(&paths.whiteboards)?;
        fs::create_dir_all(&paths.pdf)?;
        fs::create_dir_all(&paths.app)?;
        fs::create_dir_all(paths.cache.join("index"))?;
        fs::create_dir_all(paths.cache.join("thumbnails"))?;

        let config = WorkspaceConfig::default();
        let config_path = paths.app.join("config.toml");
        if !config_path.exists() {
            fs::write(
                &config_path,
                format!(
                    "workspace_version = {}\nmigration_placeholder = \"{}\"\n",
                    config.workspace_version, config.migration_placeholder
                ),
            )?;
        }

        Ok(Self { paths, config })
    }

    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref();
        if !root.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("workspace root does not exist: {}", root.display()),
            ));
        }
        let paths = canonical_paths(root);
        let config = read_config(paths.app.join("config.toml")).unwrap_or_default();
        Ok(Self { paths, config })
    }

    pub fn summary(&self) -> io::Result<WorkspaceSummary> {
        let markdown_files = collect_markdown_files(&self.paths.root)?;
        let journal_files = markdown_files
            .iter()
            .filter(|path| is_journal_file(path))
            .count();
        let page_files = markdown_files
            .iter()
            .filter(|path| {
                path.parent() == Some(self.paths.pages.as_path())
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .and_then(page_key_from_page_file)
                        .is_some()
            })
            .count();
        let asset_files = count_files(&self.paths.assets)?;
        let issues = detect_compatibility_issues(&markdown_files, &self.paths.root)?;
        let supported_paths = vec![
            self.paths.journals.clone(),
            self.paths.pages.clone(),
            self.paths.assets.clone(),
            self.paths.whiteboards.clone(),
            self.paths.pdf.clone(),
            self.paths.app.clone(),
            self.paths.cache.clone(),
        ];

        Ok(WorkspaceSummary {
            root: self.paths.root.clone(),
            journal_files,
            page_files,
            asset_files,
            supported_paths,
            issues,
        })
    }

    pub fn workspace_markdown_files(&self) -> io::Result<Vec<PathBuf>> {
        collect_markdown_files(&self.paths.root)
    }
}

pub fn canonical_paths(root: &Path) -> WorkspacePaths {
    WorkspacePaths {
        root: root.to_path_buf(),
        journals: root.join("journals"),
        pages: root.join("pages"),
        assets: root.join("assets"),
        whiteboards: root.join("whiteboards"),
        pdf: root.join("pdf"),
        app: root.join("app"),
        cache: root.join(".cache"),
    }
}

pub fn is_journal_file(path: &Path) -> bool {
    path.parent().and_then(|parent| parent.file_name()) == Some("journals".as_ref())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(JournalDate::from_journal_file_name)
            .is_some()
}

pub fn read_config(path: impl AsRef<Path>) -> io::Result<WorkspaceConfig> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(WorkspaceConfig::default());
    }
    let content = fs::read_to_string(path)?;
    let mut config = WorkspaceConfig::default();
    for line in content.lines() {
        let mut parts = line.splitn(2, '=');
        let key = parts.next().map(str::trim);
        let value = parts.next().map(str::trim);
        match (key, value) {
            (Some("workspace_version"), Some(value)) => {
                config.workspace_version = value.parse().unwrap_or(config.workspace_version);
            }
            (Some("migration_placeholder"), Some(value)) => {
                config.migration_placeholder = value.trim_matches('"').to_string();
            }
            _ => {}
        }
    }
    Ok(config)
}

pub fn detect_compatibility_issues(
    markdown_files: &[PathBuf],
    root: &Path,
) -> io::Result<Vec<CompatibilityIssue>> {
    let mut issues = Vec::new();
    let patterns = [
        (
            "manual block ref",
            CompatibilitySeverity::Unsupported,
            "((",
            "manual block refs are intentionally unsupported",
        ),
        (
            "manual block embed",
            CompatibilitySeverity::Unsupported,
            "{{embed",
            "block embeds are intentionally unsupported",
        ),
        (
            "durable block id",
            CompatibilitySeverity::Degraded,
            "id::",
            "durable block IDs degrade to file-and-span anchors",
        ),
        (
            "advanced query",
            CompatibilitySeverity::Degraded,
            "{{query",
            "advanced Logseq queries are not first-class in the markdown-first engine",
        ),
    ];

    for file in markdown_files {
        let content = fs::read_to_string(file)?;
        let relative_path = file.strip_prefix(root).unwrap_or(file).to_path_buf();
        for (construct, severity, pattern, message) in &patterns {
            if content.contains(pattern) {
                issues.push(CompatibilityIssue {
                    severity: severity.clone(),
                    relative_path: relative_path.clone(),
                    construct: (*construct).to_string(),
                    message: (*message).to_string(),
                });
            }
        }
    }
    Ok(issues)
}

fn collect_markdown_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_markdown_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown_files_inner(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(".git") {
            continue;
        }
        if path.is_dir() {
            collect_markdown_files_inner(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(())
}

fn count_files(root: &Path) -> io::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count += count_files(&path)?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

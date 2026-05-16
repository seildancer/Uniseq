use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, supported_workspace_markdown_path};

pub(crate) const PAGES_ROOT: &str = "pages";
pub(crate) const ASSETS_ROOT: &str = "assets";
pub(crate) const UNISEQ_ROOT: &str = "uniseq";
const STANDARD_WORKSPACE_FOLDERS: [&str; 5] = ["pages", "assets", "uniseq", "journals", "diary"];

pub fn prepare_workspace_root(root: impl AsRef<Path>) -> Result<(), CoreError> {
    let root = root.as_ref();
    ensure_standard_workspace_folders(root)
}

pub fn create_workspace_root(
    parent_path: impl AsRef<Path>,
    folder_name: &str,
) -> Result<PathBuf, CoreError> {
    let folder_name = validate_workspace_folder_name(folder_name)?;
    let parent_path = parent_path.as_ref();
    ensure_directory_exists(parent_path)?;

    let root_path = parent_path.join(folder_name);
    if root_path.exists() {
        if root_path.is_dir() && looks_like_workspace_root(&root_path) {
            prepare_workspace_root(&root_path)?;
            return Ok(root_path);
        }

        return Err(CoreError::WorkspaceTargetExists { path: root_path });
    }

    prepare_workspace_root(&root_path)?;
    Ok(root_path)
}

pub fn validate_workspace_folder_name(input: &str) -> Result<String, CoreError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidWorkspaceName {
            message: "workspace folder name cannot be empty".to_owned(),
        });
    }

    if matches!(trimmed, "." | "..") {
        return Err(CoreError::InvalidWorkspaceName {
            message: "workspace folder name cannot be '.' or '..'".to_owned(),
        });
    }

    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err(CoreError::InvalidWorkspaceName {
            message: "workspace folder name cannot end with a dot or space".to_owned(),
        });
    }

    for ch in trimmed.chars() {
        if ch.is_control() {
            return Err(CoreError::InvalidWorkspaceName {
                message: format!(
                    "workspace folder name contains control character U+{:04X}",
                    ch as u32
                ),
            });
        }

        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            return Err(CoreError::InvalidWorkspaceName {
                message: format!("workspace folder name contains reserved character '{ch}'"),
            });
        }
    }

    let upper = trimmed.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        return Err(CoreError::InvalidWorkspaceName {
            message: "workspace folder name cannot be a reserved Windows device name".to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

pub(crate) fn validate_workspace_root(root: &Path) -> Result<(), CoreError> {
    let pages_path = root.join(PAGES_ROOT);
    if !pages_path.exists() {
        return Err(CoreError::InvalidWorkspaceStructure {
            path: pages_path,
            message: "workspace must contain a pages/ directory".to_owned(),
        });
    }
    if !pages_path.is_dir() {
        return Err(CoreError::InvalidWorkspaceStructure {
            path: pages_path,
            message: "pages/ must be a directory".to_owned(),
        });
    }

    validate_pages_directory(root, &pages_path)
}

pub(crate) fn collect_supported_workspace_markdown_paths(
    root: &Path,
) -> Result<Vec<PathBuf>, CoreError> {
    validate_workspace_root(root)?;

    let mut markdown_paths = Vec::new();
    collect_pages_markdown_paths(root, &root.join(PAGES_ROOT), &mut markdown_paths)?;

    let mut entries = fs::read_dir(root).map_err(|error| CoreError::io(root, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(root, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;
        if !file_type.is_dir() {
            continue;
        }

        let Some(root_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_reserved_root_name(&root_name) {
            continue;
        }
        if !is_stream_directory(&entry_path)? {
            continue;
        }

        collect_stream_markdown_paths(root, &entry_path, &mut markdown_paths)?;
    }

    Ok(markdown_paths)
}

pub(crate) fn all_stream_names(root: &Path) -> Result<Vec<String>, CoreError> {
    validate_workspace_root(root)?;

    let mut stream_names = Vec::new();
    let mut entries = fs::read_dir(root).map_err(|error| CoreError::io(root, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(root, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;
        if !file_type.is_dir() {
            continue;
        }

        let Some(root_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_reserved_root_name(&root_name) {
            continue;
        }
        if !is_stream_directory(&entry_path)? {
            continue;
        }

        stream_names.push(root_name);
    }

    stream_names.sort();
    Ok(stream_names)
}

pub(crate) fn is_supported_workspace_markdown_path(
    root: &Path,
    relative_path: &Path,
) -> Result<bool, CoreError> {
    let Some(resolved) = supported_workspace_markdown_path(relative_path)? else {
        return Ok(false);
    };

    match resolved.location {
        super::PageLocation::Pages => {
            validate_pages_directory(root, &root.join(PAGES_ROOT))?;
            Ok(true)
        }
        super::PageLocation::Stream { stream_name } => {
            let stream_path = root.join(stream_name.as_str());
            Ok(stream_path.is_dir() && is_stream_directory(&stream_path)?)
        }
    }
}

pub(crate) fn is_reserved_root_name(name: &str) -> bool {
    matches!(name, PAGES_ROOT | ASSETS_ROOT | UNISEQ_ROOT)
}

pub(crate) fn is_stream_date_markdown_name(file_name: &str) -> bool {
    let Some(stem) = strip_markdown_extension(file_name) else {
        return false;
    };

    is_stream_date_name(stem)
}

pub(crate) fn is_stream_date_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'_'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'_'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn validate_pages_directory(root: &Path, pages_path: &Path) -> Result<(), CoreError> {
    let mut entries =
        fs::read_dir(pages_path).map_err(|error| CoreError::io(pages_path, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(pages_path, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;

        if file_type.is_dir() {
            return Err(CoreError::InvalidWorkspaceStructure {
                path: relative_to_root(root, &entry_path)?,
                message: "pages/ cannot contain subdirectories".to_owned(),
            });
        }
        if !file_type.is_file() {
            continue;
        }
        if !is_markdown_file(&entry_path) {
            return Err(CoreError::InvalidWorkspaceStructure {
                path: relative_to_root(root, &entry_path)?,
                message: "pages/ can only contain markdown files".to_owned(),
            });
        }
    }

    Ok(())
}

fn collect_pages_markdown_paths(
    root: &Path,
    pages_path: &Path,
    markdown_paths: &mut Vec<PathBuf>,
) -> Result<(), CoreError> {
    let mut entries =
        fs::read_dir(pages_path).map_err(|error| CoreError::io(pages_path, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(pages_path, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;
        if !file_type.is_file() || !is_markdown_file(&entry_path) {
            continue;
        }

        markdown_paths.push(relative_to_root(root, &entry_path)?);
    }

    Ok(())
}

fn collect_stream_markdown_paths(
    root: &Path,
    stream_path: &Path,
    markdown_paths: &mut Vec<PathBuf>,
) -> Result<(), CoreError> {
    let mut entries =
        fs::read_dir(stream_path).map_err(|error| CoreError::io(stream_path, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(stream_path, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;
        if !file_type.is_file() || !is_markdown_file(&entry_path) {
            continue;
        }

        markdown_paths.push(relative_to_root(root, &entry_path)?);
    }

    Ok(())
}

fn is_stream_directory(stream_path: &Path) -> Result<bool, CoreError> {
    let mut entries =
        fs::read_dir(stream_path).map_err(|error| CoreError::io(stream_path, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(stream_path, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;

        if file_type.is_dir() {
            return Ok(false);
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(false);
        };
        if !is_stream_date_markdown_name(&file_name) {
            return Ok(false);
        }
    }

    Ok(true)
}

fn ensure_directory_exists(path: &Path) -> Result<(), CoreError> {
    if !path.exists() {
        return Err(CoreError::WorkspaceParentMissing {
            path: path.to_path_buf(),
        });
    }
    if !path.is_dir() {
        return Err(CoreError::WorkspaceParentNotDirectory {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn looks_like_workspace_root(path: &Path) -> bool {
    path.join(PAGES_ROOT).is_dir()
}

fn ensure_standard_workspace_folders(root: &Path) -> Result<(), CoreError> {
    for folder_name in STANDARD_WORKSPACE_FOLDERS {
        let folder_path = root.join(folder_name);
        if folder_path.exists() {
            if !folder_path.is_dir() {
                return Err(CoreError::InvalidWorkspaceStructure {
                    path: folder_path,
                    message: format!("{folder_name}/ must be a directory"),
                });
            }
            continue;
        }

        fs::create_dir_all(&folder_path).map_err(|error| CoreError::io(&folder_path, &error))?;
    }

    Ok(())
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn strip_markdown_extension(file_name: &str) -> Option<&str> {
    let (stem, ext) = file_name.rsplit_once('.')?;
    ext.eq_ignore_ascii_case("md").then_some(stem)
}

fn relative_to_root(root: &Path, path: &Path) -> Result<PathBuf, CoreError> {
    path.strip_prefix(root).map(Path::to_path_buf).map_err(|_| {
        CoreError::io(
            root,
            &std::io::Error::from(std::io::ErrorKind::InvalidInput),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        all_stream_names, create_workspace_root, prepare_workspace_root,
        validate_workspace_folder_name,
    };
    use crate::core::files::TestWorkspace;

    #[test]
    fn lists_only_valid_stream_directories() {
        let workspace = TestWorkspace::new("uniseq-storage");
        std::fs::create_dir_all(workspace.root.join("journal")).unwrap();
        std::fs::create_dir_all(workspace.root.join("scratch")).unwrap();
        std::fs::create_dir_all(workspace.root.join("nested")).unwrap();
        std::fs::create_dir_all(workspace.root.join("pages")).unwrap();

        workspace.write_raw_file("scratch/notes.txt", "");
        workspace.write_raw_file("nested/2026_05_08.md", "");
        std::fs::create_dir_all(workspace.root.join("nested").join("archive")).unwrap();

        assert_eq!(all_stream_names(&workspace.root).unwrap(), vec!["journal"]);
    }

    #[test]
    fn create_workspace_root_reopens_existing_workspace_root() {
        let root = std::env::temp_dir().join("uniseq-storage-existing-root");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Notebook").join("pages")).unwrap();
        std::fs::write(root.join("Notebook").join("pages").join("A.md"), "").unwrap();

        let workspace_root = create_workspace_root(&root, "Notebook").unwrap();

        assert_eq!(workspace_root, root.join("Notebook"));
        assert!(workspace_root.join("assets").is_dir());
        assert!(workspace_root.join("uniseq").is_dir());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prepare_workspace_root_backfills_standard_directories() {
        let workspace = TestWorkspace::new("uniseq-storage-prepare");

        prepare_workspace_root(&workspace.root).unwrap();

        assert!(workspace.root.join("pages").is_dir());
        assert!(workspace.root.join("assets").is_dir());
        assert!(workspace.root.join("uniseq").is_dir());
        assert!(workspace.root.join("journals").is_dir());
        assert!(workspace.root.join("diary").is_dir());
    }

    #[test]
    fn validate_workspace_folder_name_rejects_invalid_values() {
        assert!(validate_workspace_folder_name("  ").is_err());
        assert!(validate_workspace_folder_name("bad/name").is_err());
        assert!(validate_workspace_folder_name("COM1").is_err());
    }
}

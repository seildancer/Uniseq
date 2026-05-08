use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, supported_workspace_markdown_path};

pub(crate) const PAGES_ROOT: &str = "pages";
pub(crate) const ASSETS_ROOT: &str = "assets";
pub(crate) const UNISEQ_ROOT: &str = "uniseq";

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
    let mut entries = fs::read_dir(pages_path).map_err(|error| CoreError::io(pages_path, &error))?;
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
    let mut entries = fs::read_dir(pages_path).map_err(|error| CoreError::io(pages_path, &error))?;
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
    let mut entries = fs::read_dir(stream_path).map_err(|error| CoreError::io(stream_path, &error))?;
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
    let mut entries = fs::read_dir(stream_path).map_err(|error| CoreError::io(stream_path, &error))?;
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
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| CoreError::io(root, &std::io::Error::from(std::io::ErrorKind::InvalidInput)))
}

use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, supported_workspace_markdown_path};

pub(crate) const PAGES_ROOT: &str = "pages";
pub(crate) const STREAMS_ROOT: &str = "streams";

pub(crate) fn supported_workspace_root_names() -> [&'static str; 2] {
    [PAGES_ROOT, STREAMS_ROOT]
}

pub(crate) fn collect_supported_workspace_markdown_paths(
    root: &Path,
) -> Result<Vec<PathBuf>, CoreError> {
    let mut markdown_paths = Vec::new();

    for root_name in supported_workspace_root_names() {
        let root_path = root.join(root_name);
        if !root_path.exists() {
            continue;
        }

        collect_markdown_paths_in_dir(root, &root_path, &mut markdown_paths)?;
    }

    Ok(markdown_paths)
}

fn collect_markdown_paths_in_dir(
    root: &Path,
    current_dir: &Path,
    markdown_paths: &mut Vec<PathBuf>,
) -> Result<(), CoreError> {
    let mut entries =
        fs::read_dir(current_dir).map_err(|error| CoreError::io(current_dir, &error))?;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|error| CoreError::io(current_dir, &error))?
    {
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CoreError::io(entry_path.clone(), &error))?;

        if file_type.is_dir() {
            collect_markdown_paths_in_dir(root, &entry_path, markdown_paths)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let is_markdown = entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
        if !is_markdown {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(root)
            .map_err(|_| {
                CoreError::io(
                    root,
                    &std::io::Error::from(std::io::ErrorKind::InvalidInput),
                )
            })?
            .to_path_buf();

        if supported_workspace_markdown_path(&relative_path)?.is_some() {
            markdown_paths.push(relative_path);
        }
    }

    Ok(())
}

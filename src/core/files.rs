use std::fs;
use std::path::{Path, PathBuf};

use super::storage;
use super::{
    CoreError, FileFingerprint, Page, WorkspaceCache, discover_workspace, parse_blocks,
    resolve_workspace_path,
};

pub(crate) fn load_workspace_cache(root: impl AsRef<Path>) -> Result<WorkspaceCache, CoreError> {
    Ok(discover_workspace(root)?.cache)
}

pub(crate) fn collect_supported_workspace_markdown_paths(
    root: &Path,
) -> Result<Vec<PathBuf>, CoreError> {
    storage::collect_supported_workspace_markdown_paths(root)
}

pub(crate) fn load_page_from_relative_path(
    root: &Path,
    relative_path: &Path,
) -> Result<Page, CoreError> {
    Ok(load_page_with_fingerprint_from_relative_path(root, relative_path)?.0)
}

pub(crate) fn load_page_with_fingerprint_from_relative_path(
    root: &Path,
    relative_path: &Path,
) -> Result<(Page, FileFingerprint), CoreError> {
    let absolute_path = root.join(relative_path);
    let text =
        fs::read_to_string(&absolute_path).map_err(|error| CoreError::io(absolute_path, &error))?;
    page_and_fingerprint_from_text(relative_path, text)
}

pub(crate) fn page_from_markdown_in_location(
    page_id: super::PageId,
    location: super::PageLocation,
    text: String,
) -> Result<Page, CoreError> {
    let blocks = parse_blocks(&text)?;
    Ok(Page::new_in_location(page_id, location, text)?.with_blocks(blocks))
}

pub(crate) fn page_and_fingerprint_from_text(
    relative_path: &Path,
    text: String,
) -> Result<(Page, FileFingerprint), CoreError> {
    let resolved = resolve_workspace_path(relative_path)?;
    let fingerprint = FileFingerprint::from_text(&text);
    let page = page_from_markdown_in_location(resolved.page_id, resolved.location, text)?;
    Ok((page, fingerprint))
}

#[cfg(test)]
pub(crate) fn workspace_test_relative_path(relative_path: &str) -> PathBuf {
    let path = PathBuf::from(relative_path);
    let is_top_level_markdown = path.components().count() == 1
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    if is_top_level_markdown {
        PathBuf::from("pages").join(path)
    } else {
        path
    }
}

#[cfg(test)]
pub(crate) struct TestWorkspace {
    pub(crate) root: PathBuf,
}

#[cfg(test)]
impl TestWorkspace {
    pub(crate) fn new(prefix: &str) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("uniseq")).unwrap();
        Self { root }
    }

    pub(crate) fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(workspace_test_relative_path(relative_path));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    pub(crate) fn write_raw_file(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    pub(crate) fn remove_file(&self, relative_path: &str) {
        fs::remove_file(self.root.join(workspace_test_relative_path(relative_path))).unwrap();
    }

    pub(crate) fn read_file(&self, relative_path: &str) -> String {
        fs::read_to_string(self.root.join(workspace_test_relative_path(relative_path))).unwrap()
    }

    pub(crate) fn file_exists(&self, relative_path: &str) -> bool {
        self.root
            .join(workspace_test_relative_path(relative_path))
            .exists()
    }
}

#[cfg(test)]
impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

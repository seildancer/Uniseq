use std::fs;
use std::path::{Path, PathBuf};

use super::{CoreError, FileFingerprint, Page, PageId, WorkspaceCache, parse_blocks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDiscovery {
    pub cache: WorkspaceCache,
    pub missing_parent_page_ids: Vec<PageId>,
}

pub fn discover_workspace(root: impl AsRef<Path>) -> Result<WorkspaceDiscovery, CoreError> {
    let root = root.as_ref();
    let mut markdown_paths = Vec::new();
    collect_markdown_paths(root, root, &mut markdown_paths)?;
    markdown_paths.sort();

    let mut pages = Vec::with_capacity(markdown_paths.len());
    for relative_path in markdown_paths {
        pages.push(load_page_from_relative_path(root, &relative_path)?);
    }

    let cache = WorkspaceCache::from_pages(pages);
    let missing_parent_page_ids = cache.missing_parent_page_ids();

    Ok(WorkspaceDiscovery {
        cache,
        missing_parent_page_ids,
    })
}

pub fn materialize_parent_pages<I>(
    root: impl AsRef<Path>,
    cache: &mut WorkspaceCache,
    page_ids: I,
) -> Result<Vec<PageId>, CoreError>
where
    I: IntoIterator<Item = PageId>,
{
    let root = root.as_ref();
    let mut created_or_loaded = Vec::new();

    for page_id in page_ids {
        if cache.page(&page_id).is_some() {
            continue;
        }

        let relative_path = page_id.to_workspace_path();
        let absolute_path = root.join(&relative_path);

        if absolute_path.exists() {
            let page = load_page_from_relative_path(root, &relative_path)?;
            cache.upsert_page(page);
            created_or_loaded.push(page_id);
            continue;
        }

        fs::write(&absolute_path, "")
            .map_err(|error| CoreError::io(absolute_path.clone(), &error))?;
        cache.upsert_page(Page::new(page_id.clone(), FileFingerprint::from_text("")));
        created_or_loaded.push(page_id);
    }

    Ok(created_or_loaded)
}

fn collect_markdown_paths(
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
            collect_markdown_paths(root, &entry_path, markdown_paths)?;
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

        markdown_paths.push(relative_path);
    }

    Ok(())
}

fn load_page_from_relative_path(root: &Path, relative_path: &Path) -> Result<Page, CoreError> {
    let page_id = PageId::from_workspace_path(relative_path)?;
    let absolute_path = root.join(relative_path);
    let text =
        fs::read_to_string(&absolute_path).map_err(|error| CoreError::io(absolute_path, &error))?;
    let blocks = parse_blocks(&text)?;

    Ok(Page::new(page_id, FileFingerprint::from_text(&text)).with_blocks(blocks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("uniseq-discovery-{unique}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_file(&self, relative_path: &str, contents: &str) {
            let path = self.root.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn discovers_flat_markdown_pages_into_a_page_tree() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "");
        workspace.write_file("A___B.md", "");
        workspace.write_file("A___B___C.md", "");
        workspace.write_file("ignored.txt", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert_eq!(
            discovery
                .cache
                .pages()
                .keys()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B", "A/B/C"]
        );
        assert!(discovery.missing_parent_page_ids.is_empty());

        let root_page = discovery.cache.page(&PageId::new(["A"]).unwrap()).unwrap();
        assert_eq!(
            root_page.child_page_ids,
            vec![PageId::new(["A", "B"]).unwrap()]
        );

        let middle_page = discovery
            .cache
            .page(&PageId::new(["A", "B"]).unwrap())
            .unwrap();
        assert_eq!(
            middle_page.child_page_ids,
            vec![PageId::new(["A", "B", "C"]).unwrap()]
        );
    }

    #[test]
    fn reports_missing_parent_pages_without_materializing_them() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A___B___C.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert_eq!(
            discovery.missing_parent_page_ids,
            vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap()
            ]
        );
        assert_eq!(discovery.cache.pages().len(), 1);
    }

    #[test]
    fn preserves_exact_file_fingerprint_during_discovery() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- A\r\n");
        workspace.write_file("B.md", "- A\n");

        let discovery = discover_workspace(&workspace.root).unwrap();

        let a = discovery.cache.page(&PageId::new(["A"]).unwrap()).unwrap();
        let b = discovery.cache.page(&PageId::new(["B"]).unwrap()).unwrap();
        assert_ne!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn rejects_nested_markdown_files() {
        let workspace = TestWorkspace::new();
        workspace.write_file("notes/A.md", "");

        let error = discover_workspace(&workspace.root).unwrap_err();
        assert_eq!(
            error,
            CoreError::InvalidPagePath(super::super::PagePathError::NestedPath)
        );
    }

    #[test]
    fn ignores_non_markdown_files() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.txt", "");
        workspace.write_file("B.json", "");

        let discovery = discover_workspace(&workspace.root).unwrap();
        assert!(discovery.cache.pages().is_empty());
    }

    #[test]
    fn orders_discovered_pages_deterministically() {
        let workspace = TestWorkspace::new();
        workspace.write_file("B.md", "");
        workspace.write_file("A___B.md", "");
        workspace.write_file("A.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert_eq!(
            discovery
                .cache
                .pages()
                .keys()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B", "B"]
        );
    }

    #[test]
    fn materializes_missing_parent_pages_as_empty_files() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A___B___C.md", "");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let created = materialize_parent_pages(
            &workspace.root,
            &mut discovery.cache,
            discovery.missing_parent_page_ids.clone(),
        )
        .unwrap();

        assert_eq!(
            created
                .iter()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B"]
        );
        assert!(workspace.root.join("A.md").exists());
        assert!(workspace.root.join("A___B.md").exists());
        assert!(
            discover_workspace(&workspace.root)
                .unwrap()
                .missing_parent_page_ids
                .is_empty()
        );
    }

    #[test]
    fn materialize_parent_pages_does_not_overwrite_existing_files() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A___B___C.md", "");
        workspace.write_file("A.md", "existing");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let created = materialize_parent_pages(
            &workspace.root,
            &mut discovery.cache,
            discovery.missing_parent_page_ids.clone(),
        )
        .unwrap();

        assert_eq!(
            created
                .iter()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A/B"]
        );
        assert_eq!(
            fs::read_to_string(workspace.root.join("A.md")).unwrap(),
            "existing"
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .fingerprint,
            FileFingerprint::from_text("existing")
        );
    }
}

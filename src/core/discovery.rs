use std::fs;
use std::path::{Path, PathBuf};

use super::files::load_page_from_relative_path;
use super::{CoreError, Page, PageId, WorkspaceCache};

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
        cache.upsert_page(Page::new(page_id.clone(), ""));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BlockHandle, BlockSubtreeEdit, CoreError, FileFingerprint, WorkspaceReadApi,
        apply_block_subtree_edit,
    };
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

        fn read_file(&self, relative_path: &str) -> String {
            fs::read_to_string(self.root.join(relative_path)).unwrap()
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
    fn subtree_edit_replaces_exact_source_region_and_refreshes_cache() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- before\n- parent [[B]]\n\t- child #C\n- after\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");
        workspace.write_file("D.md", "");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let read_api = WorkspaceReadApi::new(&discovery.cache);
        let handle = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap()[1]
            .handle
            .clone();

        apply_block_subtree_edit(
            &workspace.root,
            &mut discovery.cache,
            BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- parent [[D]]\n\t- child plain\n".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(
            workspace.read_file("A.md"),
            "- before\n- parent [[D]]\n\t- child plain\n- after\n"
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .text,
            "- before\n- parent [[D]]\n\t- child plain\n- after\n"
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["D"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn subtree_edit_accepts_handles_from_linked_references() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- [[B]]\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let read_api = WorkspaceReadApi::new(&discovery.cache);
        let handle = read_api.linked_refs(&PageId::new(["B"]).unwrap()).unwrap()[0]
            .source_block
            .handle
            .clone();

        apply_block_subtree_edit(
            &workspace.root,
            &mut discovery.cache,
            BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- [[C]]\n".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(workspace.read_file("A.md"), "- [[C]]\n");
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["B"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            0
        );
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["C"]).unwrap())
                .unwrap()
                .incoming_refs
                .len(),
            1
        );
    }

    #[test]
    fn subtree_edit_rejects_stale_handles_without_mutating_file_or_cache() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- original\n");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let read_api = WorkspaceReadApi::new(&discovery.cache);
        let handle = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap()[0]
            .handle
            .clone();
        workspace.write_file("A.md", "- external change\n");

        let error = apply_block_subtree_edit(
            &workspace.root,
            &mut discovery.cache,
            BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- local edit\n".to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::StalePageRevision);
        assert_eq!(workspace.read_file("A.md"), "- external change\n");
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .text,
            "- original\n"
        );
    }

    #[test]
    fn subtree_edit_rejects_missing_block_handles() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- only\n");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let page = discovery.cache.page(&PageId::new(["A"]).unwrap()).unwrap();
        let invalid_handle = BlockHandle::new(
            PageId::new(["A"]).unwrap(),
            page.fingerprint,
            super::super::SourceSpan::unchecked(0, 3),
        );

        let error = apply_block_subtree_edit(
            &workspace.root,
            &mut discovery.cache,
            BlockSubtreeEdit {
                block_handle: invalid_handle,
                replacement_markdown: "- no\n".to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(error, CoreError::MissingBlock);
        assert_eq!(workspace.read_file("A.md"), "- only\n");
    }

    #[test]
    fn subtree_edit_preserves_cache_when_file_write_fails() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- only\n");

        let mut discovery = discover_workspace(&workspace.root).unwrap();
        let read_api = WorkspaceReadApi::new(&discovery.cache);
        let handle = read_api.page_blocks(&PageId::new(["A"]).unwrap()).unwrap()[0]
            .handle
            .clone();

        let path = workspace.root.join("A.md");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();

        let error = apply_block_subtree_edit(
            &workspace.root,
            &mut discovery.cache,
            BlockSubtreeEdit {
                block_handle: handle,
                replacement_markdown: "- changed\n".to_owned(),
            },
        )
        .unwrap_err();

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();

        assert!(matches!(error, CoreError::Io { .. }));
        assert_eq!(workspace.read_file("A.md"), "- only\n");
        assert_eq!(
            discovery
                .cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .text,
            "- only\n"
        );
    }

    #[test]
    fn discovery_populates_incoming_refs_from_parsed_markdown() {
        let workspace = TestWorkspace::new();
        workspace.write_file("A.md", "- see [[B]] and #C\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        let b = discovery.cache.page(&PageId::new(["B"]).unwrap()).unwrap();
        let c = discovery.cache.page(&PageId::new(["C"]).unwrap()).unwrap();

        assert_eq!(b.incoming_refs.len(), 1);
        assert_eq!(c.incoming_refs.len(), 1);
        assert_eq!(
            b.incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
        );
        assert_eq!(
            c.incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
        );
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

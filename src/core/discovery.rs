use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::files::{collect_supported_workspace_markdown_paths, load_page_from_relative_path};
use super::{CoreError, Page, PageId, PageLocation, WorkspaceCache};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDiscovery {
    pub cache: WorkspaceCache,
    pub missing_parent_page_ids: Vec<PageId>,
}

pub fn discover_workspace(root: impl AsRef<Path>) -> Result<WorkspaceDiscovery, CoreError> {
    let root = root.as_ref();
    println!(
        "[uniseq-backend] supported-root scan: discovering workspace at {}",
        root.display()
    );
    let mut markdown_paths = collect_supported_workspace_markdown_paths(root)?;
    markdown_paths.sort();

    let mut page_paths = BTreeMap::new();
    let mut pages = Vec::with_capacity(markdown_paths.len());
    for relative_path in markdown_paths {
        let page = load_page_from_relative_path(root, &relative_path)?;
        if page_paths
            .insert(page.page_id.clone(), relative_path.clone())
            .is_some()
        {
            return Err(CoreError::DuplicatePageIdentity {
                page_id: page.page_id.canonical_identity_display(),
            });
        }
        pages.push(page);
    }

    let mut cache = WorkspaceCache::from_pages(pages);
    let missing_parent_page_ids = cache.missing_parent_page_ids();
    if !missing_parent_page_ids.is_empty() {
        materialize_parent_pages(root, &mut cache, missing_parent_page_ids.clone())?;
    }
    let missing_ref_page_ids = missing_referenced_page_ids(&cache);
    if !missing_ref_page_ids.is_empty() {
        materialize_parent_pages(root, &mut cache, missing_ref_page_ids)?;
    }
    let remaining_missing_parent_page_ids = cache.missing_parent_page_ids();
    println!(
        "[uniseq-backend] supported-root scan complete: {} supported markdown pages discovered",
        cache.pages().len()
    );

    Ok(WorkspaceDiscovery {
        cache,
        missing_parent_page_ids: remaining_missing_parent_page_ids,
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

        let relative_path = PageLocation::Pages.workspace_path_for_page_id(&page_id)?;
        let absolute_path = root.join(&relative_path);

        if absolute_path.exists() {
            let page = load_page_from_relative_path(root, &relative_path)?;
            cache.upsert_page(page);
            created_or_loaded.push(page_id);
            continue;
        }

        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, &error))?;
        }
        fs::write(&absolute_path, "")
            .map_err(|error| CoreError::io(absolute_path.clone(), &error))?;
        cache.upsert_page(Page::new(page_id.clone(), ""));
        created_or_loaded.push(page_id);
    }

    Ok(created_or_loaded)
}

fn missing_referenced_page_ids(cache: &WorkspaceCache) -> Vec<PageId> {
    let mut missing = std::collections::BTreeSet::new();
    for page in cache.pages().values() {
        for outgoing_ref in page.outgoing_refs() {
            let target = &outgoing_ref.target_page_id;
            if target.is_page_backed() && cache.page(target).is_none() {
                missing.insert(target.clone());
            }
        }
    }
    missing.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileFingerprint,
        core::files::{TestWorkspace, workspace_test_relative_path},
    };

    #[test]
    fn discovers_flat_markdown_pages_into_a_page_tree() {
        let workspace = TestWorkspace::new("uniseq-discovery");
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
    fn materializes_missing_parent_pages_during_discovery() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A___B___C.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert!(discovery.missing_parent_page_ids.is_empty());
        assert_eq!(discovery.cache.pages().len(), 3);
        assert!(
            workspace
                .root
                .join(workspace_test_relative_path("A.md"))
                .exists()
        );
        assert!(
            workspace
                .root
                .join(workspace_test_relative_path("A___B.md"))
                .exists()
        );
    }

    #[test]
    fn preserves_exact_file_fingerprint_during_discovery() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A.md", "- A\r\n");
        workspace.write_file("B.md", "- A\n");

        let discovery = discover_workspace(&workspace.root).unwrap();

        let a = discovery.cache.page(&PageId::new(["A"]).unwrap()).unwrap();
        let b = discovery.cache.page(&PageId::new(["B"]).unwrap()).unwrap();
        assert_ne!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn discovery_populates_incoming_refs_from_parsed_markdown() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A.md", "- see [[B]] and #C\n");
        workspace.write_file("B.md", "");
        workspace.write_file("C.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        let b_incoming_refs = discovery.cache.incoming_refs(&PageId::new(["B"]).unwrap());
        let c_incoming_refs = discovery.cache.incoming_refs(&PageId::new(["C"]).unwrap());

        assert_eq!(b_incoming_refs.len(), 1);
        assert_eq!(c_incoming_refs.len(), 1);
        assert_eq!(
            b_incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
        );
        assert_eq!(
            c_incoming_refs[0].source_page_id,
            PageId::new(["A"]).unwrap()
        );
    }

    #[test]
    fn discovery_keeps_page_and_stream_with_same_segments_distinct() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("journal___2026_05_07.md", "");
        workspace.write_file("journal/2026_05_07.md", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert!(
            discovery
                .cache
                .page(&PageId::new(["journal", "2026_05_07"]).unwrap())
                .is_some()
        );
        assert!(
            discovery
                .cache
                .page(
                    &PageId::stream(
                        crate::PageName::new("journal").unwrap(),
                        crate::PageName::new("2026_05_07").unwrap(),
                    )
                    .unwrap(),
                )
                .is_some()
        );
        assert!(
            discovery
                .cache
                .page(&PageId::new(["journal"]).unwrap())
                .is_some()
        );
        assert_eq!(discovery.cache.pages().len(), 3);
    }

    #[test]
    fn rejects_invalid_pages_directory_shapes() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_raw_file("pages/nested/A.md", "");

        let error = discover_workspace(&workspace.root).unwrap_err();
        assert!(matches!(error, CoreError::InvalidWorkspaceStructure { .. }));
    }

    #[test]
    fn discovery_only_scans_supported_roots() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A.md", "");
        workspace.write_file("journal/2026_05_07.md", "");
        workspace.write_raw_file("Loose.md", "");
        workspace.write_raw_file("archive/Old.md", "");
        workspace.write_raw_file("misc/readme.txt", "");

        let discovery = discover_workspace(&workspace.root).unwrap();

        assert!(discovery.cache.page(&PageId::new(["A"]).unwrap()).is_some());
        assert!(
            discovery
                .cache
                .page(
                    &PageId::stream(
                        crate::PageName::new("journal").unwrap(),
                        crate::PageName::new("2026_05_07").unwrap(),
                    )
                    .unwrap(),
                )
                .is_some()
        );
        assert_eq!(discovery.cache.pages().len(), 2);
    }

    #[test]
    fn ignores_non_markdown_files() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A.txt", "");
        workspace.write_file("B.json", "");

        let discovery = discover_workspace(&workspace.root).unwrap();
        assert!(discovery.cache.pages().is_empty());
    }

    #[test]
    fn orders_discovered_pages_deterministically() {
        let workspace = TestWorkspace::new("uniseq-discovery");
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
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A___B___C.md", "");

        let mut cache =
            WorkspaceCache::from_pages([Page::new(PageId::new(["A", "B", "C"]).unwrap(), "")]);
        let created = materialize_parent_pages(
            &workspace.root,
            &mut cache,
            vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            created
                .iter()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B"]
        );
        assert!(
            workspace
                .root
                .join(workspace_test_relative_path("A.md"))
                .exists()
        );
        assert!(
            workspace
                .root
                .join(workspace_test_relative_path("A___B.md"))
                .exists()
        );
        assert!(
            discover_workspace(&workspace.root)
                .unwrap()
                .missing_parent_page_ids
                .is_empty()
        );
    }

    #[test]
    fn materialize_parent_pages_does_not_overwrite_existing_files() {
        let workspace = TestWorkspace::new("uniseq-discovery");
        workspace.write_file("A___B___C.md", "");
        workspace.write_file("A.md", "existing");

        let mut cache =
            WorkspaceCache::from_pages([Page::new(PageId::new(["A", "B", "C"]).unwrap(), "")]);
        let created = materialize_parent_pages(
            &workspace.root,
            &mut cache,
            vec![
                PageId::new(["A"]).unwrap(),
                PageId::new(["A", "B"]).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            created
                .iter()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B"]
        );
        assert_eq!(
            fs::read_to_string(workspace.root.join(workspace_test_relative_path("A.md"))).unwrap(),
            "existing"
        );
        assert_eq!(
            cache
                .page(&PageId::new(["A"]).unwrap())
                .unwrap()
                .fingerprint,
            FileFingerprint::from_text("existing")
        );
    }
}

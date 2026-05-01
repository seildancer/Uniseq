use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::model::{
    CompatibilityIssue, DocumentKind, Edge, Entry, JournalDate, PageKey, PageRecord, ParsedDocument,
    Task,
};
use crate::parser::parse_workspace_file;
use crate::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    pub modified_millis: u128,
    pub len: u64,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceIndex {
    pub documents: BTreeMap<PathBuf, ParsedDocument>,
    pub pages: BTreeMap<PageKey, PageRecord>,
    pub incoming: BTreeMap<PageKey, Vec<Edge>>,
    pub journal_entries: BTreeMap<JournalDate, Vec<Entry>>,
    pub tasks: Vec<Task>,
    pub issues: Vec<CompatibilityIssue>,
    pub fingerprints: BTreeMap<PathBuf, FileFingerprint>,
}

impl WorkspaceIndex {
    pub fn build(workspace: &Workspace) -> io::Result<Self> {
        let mut documents = BTreeMap::new();
        let mut issues = workspace.summary()?.issues;
        let mut fingerprints = BTreeMap::new();
        for path in workspace.workspace_markdown_files()? {
            let parsed = parse_workspace_file(&workspace.paths.root, &path)?;
            issues.extend(parsed.issues.clone());
            fingerprints.insert(
                parsed.relative_path.clone(),
                file_fingerprint(&path)?,
            );
            documents.insert(parsed.relative_path.clone(), parsed);
        }

        let mut index = Self {
            documents,
            issues,
            fingerprints,
            ..Self::default()
        };
        index.rebuild_projections();
        index.persist_cache_summary(workspace)?;
        Ok(index)
    }

    pub fn rebuild_projections(&mut self) {
        self.pages.clear();
        self.incoming.clear();
        self.journal_entries.clear();
        self.tasks.clear();

        for document in self.documents.values() {
            match &document.kind {
                DocumentKind::Journal(date) => {
                    self.journal_entries
                        .entry(date.clone())
                        .or_default()
                        .extend(document.entries.clone());
                }
                DocumentKind::Page(page_key) => {
                    let page = self.pages.entry(page_key.clone()).or_insert_with(|| PageRecord {
                        key: page_key.clone(),
                        display_title: page_key.to_string(),
                        aliases: HashSet::new(),
                        has_page_file: true,
                        page_file: Some(document.relative_path.clone()),
                        namespaces: page_key
                            .namespace_segments()
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    });
                    page.has_page_file = true;
                    page.page_file = Some(document.relative_path.clone());
                    if let Some(title) = document.front_matter.values.get("title").and_then(front_matter_scalar)
                    {
                        page.display_title = title.to_string();
                    }
                    page.aliases.extend(document.front_matter.aliases());
                }
            }
        }

        let mut co_occurrence: HashMap<PageKey, BTreeSet<PageKey>> = HashMap::new();
        for document in self.documents.values() {
            for entry in &document.entries {
                let entry_pages: Vec<PageKey> = entry
                    .references
                    .iter()
                    .filter_map(|reference| reference.page_key.clone())
                    .collect();
                for task in &entry.tasks {
                    self.tasks.push(task.clone());
                }
                for page_key in &entry_pages {
                    self.pages.entry(page_key.clone()).or_insert_with(|| PageRecord {
                        key: page_key.clone(),
                        display_title: page_key.to_string(),
                        aliases: HashSet::new(),
                        has_page_file: false,
                        page_file: None,
                        namespaces: page_key
                            .namespace_segments()
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    });
                }
                for page_key in &entry_pages {
                    for sibling in &entry_pages {
                        if sibling != page_key {
                            co_occurrence
                                .entry(page_key.clone())
                                .or_default()
                                .insert(sibling.clone());
                        }
                    }
                    let first_reference = entry
                        .references
                        .iter()
                        .find(|reference| reference.page_key.as_ref() == Some(page_key))
                        .expect("reference should exist");
                    self.incoming.entry(page_key.clone()).or_default().push(Edge {
                        target: page_key.clone(),
                        source_anchor: entry.anchor.clone(),
                        source_path: document.relative_path.clone(),
                        source_date: match &document.kind {
                            DocumentKind::Journal(date) => Some(date.clone()),
                            DocumentKind::Page(_) => None,
                        },
                        source_page: match &document.kind {
                            DocumentKind::Page(page_key) => Some(page_key.clone()),
                            DocumentKind::Journal(_) => None,
                        },
                        source_text: entry.text.clone(),
                        kind: first_reference.kind.clone(),
                    });
                }
            }
        }
    }

    pub fn pages_related_to(&self, page_key: &PageKey) -> Vec<PageKey> {
        let mut related = BTreeSet::new();
        for edge in self.incoming.get(page_key).into_iter().flatten() {
            if let Some(document) = self.documents.get(&edge.source_path) {
                for entry in &document.entries {
                    if entry.anchor == edge.source_anchor {
                        for reference in &entry.references {
                            if let Some(other) = &reference.page_key {
                                if other != page_key {
                                    related.insert(other.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        related.into_iter().collect()
    }

    pub fn clear_cache(workspace: &Workspace) -> io::Result<()> {
        let index_dir = workspace.paths.cache.join("index");
        if index_dir.exists() {
            fs::remove_dir_all(&index_dir)?;
        }
        fs::create_dir_all(&index_dir)?;
        Ok(())
    }

    pub fn persist_cache_summary(&self, workspace: &Workspace) -> io::Result<()> {
        let index_dir = workspace.paths.cache.join("index");
        fs::create_dir_all(&index_dir)?;
        let cache_path = index_dir.join("summary.txt");
        let mut output = String::new();
        output.push_str("UNISEQ_CACHE_V1\n");
        output.push_str(&format!("documents={}\n", self.documents.len()));
        output.push_str(&format!("pages={}\n", self.pages.len()));
        output.push_str(&format!("tasks={}\n", self.tasks.len()));
        output.push_str(&format!("issues={}\n", self.issues.len()));
        for (path, fingerprint) in &self.fingerprints {
            output.push_str(&format!(
                "file\t{}\t{}\t{}\n",
                path.display(),
                fingerprint.modified_millis,
                fingerprint.len
            ));
        }
        fs::write(cache_path, output)
    }

    pub fn changed_paths(&self, workspace: &Workspace) -> io::Result<Vec<PathBuf>> {
        let mut changed = Vec::new();
        let current_files = workspace.workspace_markdown_files()?;
        let mut current_relative = BTreeSet::new();
        for file in current_files {
            let relative = file
                .strip_prefix(&workspace.paths.root)
                .unwrap_or(&file)
                .to_path_buf();
            current_relative.insert(relative.clone());
            let fingerprint = file_fingerprint(&file)?;
            if self.fingerprints.get(&relative) != Some(&fingerprint) {
                changed.push(relative);
            }
        }

        for existing in self.fingerprints.keys() {
            if !current_relative.contains(existing) {
                changed.push(existing.clone());
            }
        }

        changed.sort();
        changed.dedup();
        Ok(changed)
    }
}

fn front_matter_scalar(value: &crate::model::FrontMatterValue) -> Option<&str> {
    match value {
        crate::model::FrontMatterValue::Scalar(value) => Some(value.as_str()),
        crate::model::FrontMatterValue::List(_) => None,
    }
}

fn file_fingerprint(path: &Path) -> io::Result<FileFingerprint> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Ok(FileFingerprint {
        modified_millis: modified,
        len: metadata.len(),
    })
}

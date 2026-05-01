use std::io;
use std::path::{Path, PathBuf};

use crate::events::EngineEvent;
use crate::index::WorkspaceIndex;
use crate::model::{FrontMatterPatch, JournalDate, PageKey, PageView, SearchHit, Task, TimelineEntry, WorkspaceSummary};
use crate::query;
use crate::workspace::Workspace;
use crate::writer::{self, WriteError};

#[derive(Debug, Clone)]
pub struct Engine {
    pub workspace: Workspace,
    pub index: WorkspaceIndex,
    pub events: Vec<EngineEvent>,
}

impl Engine {
    pub fn create_workspace(path: impl AsRef<Path>) -> io::Result<Self> {
        let workspace = Workspace::create(path)?;
        let mut engine = Self {
            workspace,
            index: WorkspaceIndex::default(),
            events: Vec::new(),
        };
        engine.rebuild_index()?;
        Ok(engine)
    }

    pub fn open_workspace(path: impl AsRef<Path>) -> io::Result<Self> {
        let workspace = Workspace::open(path)?;
        let index = WorkspaceIndex::build(&workspace)?;
        let events = vec![EngineEvent::WorkspaceOpened {
            root: workspace.paths.root.clone(),
        }];
        Ok(Self {
            workspace,
            index,
            events,
        })
    }

    pub fn workspace_summary(&self) -> io::Result<WorkspaceSummary> {
        self.workspace.summary()
    }

    pub fn journal_dates(&self) -> Vec<JournalDate> {
        query::journal_dates(&self.index)
    }

    pub fn get_page_view(&self, page_key: &str) -> Option<PageView> {
        let page_key = PageKey::new(page_key)?;
        query::page_view(&self.index, &page_key)
    }

    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        query::search(&self.index, query)
    }

    pub fn tasks(&self, page_key: Option<&str>) -> Vec<Task> {
        let key = page_key.and_then(PageKey::new);
        query::open_tasks(&self.index, key.as_ref())
    }

    pub fn timeline(&self, page_key: Option<&str>) -> Vec<TimelineEntry> {
        let key = page_key.and_then(PageKey::new);
        query::timeline(&self.index, key.as_ref())
    }

    pub fn rebuild_index(&mut self) -> io::Result<()> {
        self.index = WorkspaceIndex::build(&self.workspace)?;
        self.events.push(EngineEvent::IndexRebuilt);
        self.events.push(EngineEvent::SearchIndexUpdated);
        Ok(())
    }

    pub fn scan_for_changes(&mut self) -> io::Result<Vec<PathBuf>> {
        let changed = self.index.changed_paths(&self.workspace)?;
        if !changed.is_empty() {
            self.rebuild_index()?;
            for path in &changed {
                self.events.push(EngineEvent::FileChanged { path: path.clone() });
            }
        }
        Ok(changed)
    }

    pub fn clear_cache(&mut self) -> io::Result<()> {
        WorkspaceIndex::clear_cache(&self.workspace)?;
        self.events.push(EngineEvent::CacheCleared);
        Ok(())
    }

    pub fn append_journal_entry(
        &mut self,
        date: &JournalDate,
        markdown: &str,
    ) -> Result<Vec<EngineEvent>, WriteError> {
        let path = writer::append_journal_entry(&self.workspace, date, markdown)?;
        self.rebuild_after_write(vec![path])?;
        Ok(self.events.clone())
    }

    pub fn edit_markdown_span(
        &mut self,
        anchor: &crate::model::SourceAnchor,
        expected_snippet: &str,
        replacement: &str,
    ) -> Result<Vec<EngineEvent>, WriteError> {
        writer::edit_markdown_span(anchor, expected_snippet, replacement)?;
        self.rebuild_after_write(vec![anchor.file_path.clone()])?;
        Ok(self.events.clone())
    }

    pub fn toggle_task(&mut self, task: &Task, checked: bool) -> Result<Vec<EngineEvent>, WriteError> {
        writer::toggle_task(task, checked)?;
        self.rebuild_after_write(vec![task.anchor.file_path.clone()])?;
        Ok(self.events.clone())
    }

    pub fn rename_page(&mut self, old_key: &str, new_key: &str) -> Result<Vec<EngineEvent>, WriteError> {
        let old_key = PageKey::new(old_key)
            .ok_or_else(|| WriteError::InvalidInput("invalid old page key".to_string()))?;
        let new_key = PageKey::new(new_key)
            .ok_or_else(|| WriteError::InvalidInput("invalid new page key".to_string()))?;
        let changed = writer::rename_page(&self.workspace, &old_key, &new_key)?;
        self.rebuild_after_write(changed)?;
        Ok(self.events.clone())
    }

    pub fn update_page_front_matter(
        &mut self,
        page_key: &str,
        patch: &FrontMatterPatch,
    ) -> Result<Vec<EngineEvent>, WriteError> {
        let page_key = PageKey::new(page_key)
            .ok_or_else(|| WriteError::InvalidInput("invalid page key".to_string()))?;
        let path = writer::update_page_front_matter(&self.workspace, &page_key, patch)?;
        self.rebuild_after_write(vec![path])?;
        Ok(self.events.clone())
    }

    pub fn move_asset(
        &mut self,
        old_relative_asset_path: impl AsRef<Path>,
        new_relative_asset_path: impl AsRef<Path>,
    ) -> Result<Vec<EngineEvent>, WriteError> {
        let changed = writer::move_asset(
            &self.workspace,
            old_relative_asset_path.as_ref(),
            new_relative_asset_path.as_ref(),
        )?;
        self.rebuild_after_write(changed)?;
        Ok(self.events.clone())
    }

    fn rebuild_after_write(&mut self, changed_paths: Vec<PathBuf>) -> Result<(), WriteError> {
        self.index = WorkspaceIndex::build(&self.workspace)?;
        for path in changed_paths {
            let relative = path
                .strip_prefix(&self.workspace.paths.root)
                .unwrap_or(&path)
                .to_path_buf();
            self.events.push(EngineEvent::FileChanged { path: relative });
        }
        self.events.push(EngineEvent::IndexRebuilt);
        self.events.push(EngineEvent::SearchIndexUpdated);
        Ok(())
    }
}

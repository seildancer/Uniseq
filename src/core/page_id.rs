use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use super::storage::{
    PAGES_ROOT, is_reserved_root_name, is_stream_date_markdown_name, is_stream_date_name,
};
use super::{NameError, PagePathError};

const HIERARCHY_DELIMITER: &str = "___";
const MARKDOWN_EXTENSION: &str = ".md";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageName(String);

impl PageName {
    pub fn new(value: impl Into<String>) -> Result<Self, NameError> {
        let value = value.into();
        validate_page_name(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PageName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for PageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PageName {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for PageName {
    type Error = NameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageLocation {
    Pages,
    Stream { stream_name: PageName },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId {
    location: PageLocation,
    segments: Vec<PageName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWorkspacePath {
    pub page_id: PageId,
    pub location: PageLocation,
}

impl PageId {
    pub fn new<I, S>(segments: I) -> Result<Self, PagePathError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new_in_location(PageLocation::Pages, segments)
    }

    pub fn new_in_location<I, S>(location: PageLocation, segments: I) -> Result<Self, PagePathError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let segments = segments
            .into_iter()
            .map(|segment| PageName::new(segment.as_ref()).map_err(PagePathError::from))
            .collect::<Result<Vec<_>, _>>()?;

        Self::from_page_names_in_location(location, segments)
    }

    pub fn from_page_names(segments: Vec<PageName>) -> Result<Self, PagePathError> {
        Self::from_page_names_in_location(PageLocation::Pages, segments)
    }

    pub fn stream(stream_name: PageName, date_name: PageName) -> Result<Self, PagePathError> {
        Self::from_page_names_in_location(
            PageLocation::Stream {
                stream_name: stream_name.clone(),
            },
            vec![stream_name, date_name],
        )
    }

    pub fn from_page_names_in_location(
        location: PageLocation,
        segments: Vec<PageName>,
    ) -> Result<Self, PagePathError> {
        if segments.is_empty() {
            return Err(PagePathError::EmptyPageId);
        }

        if let PageLocation::Stream { stream_name } = &location {
            if segments.len() != 2
                || segments.first() != Some(stream_name)
                || !is_stream_date_name(segments[1].as_str())
            {
                return Err(PagePathError::NestedPath);
            }
        }

        Ok(Self { location, segments })
    }

    pub fn from_workspace_path(path: impl AsRef<Path>) -> Result<Self, PagePathError> {
        Ok(resolve_workspace_path(path)?.page_id)
    }

    pub fn to_workspace_path(&self) -> PathBuf {
        self.location
            .workspace_path_for_page_id(self)
            .expect("workspace paths are always valid for resolved page ids")
    }

    pub fn location(&self) -> &PageLocation {
        &self.location
    }

    pub fn is_page_backed(&self) -> bool {
        self.location.is_page_backed()
    }

    pub fn is_stream_backed(&self) -> bool {
        self.location.is_stream_backed()
    }

    pub fn segments(&self) -> &[PageName] {
        &self.segments
    }

    pub fn leaf_name(&self) -> &PageName {
        self.segments
            .last()
            .expect("PageId invariant guarantees at least one segment")
    }

    pub fn parent(&self) -> Option<Self> {
        if !self.is_page_backed() {
            return None;
        }

        let parent_len = self.segments.len().checked_sub(1)?;
        if parent_len == 0 {
            return None;
        }

        Some(Self {
            location: self.location.clone(),
            segments: self.segments[..parent_len].to_vec(),
        })
    }

    pub fn ancestors(&self) -> Vec<Self> {
        if !self.is_page_backed() {
            return Vec::new();
        }

        (1..self.segments.len())
            .map(|len| Self {
                location: self.location.clone(),
                segments: self.segments[..len].to_vec(),
            })
            .collect()
    }

    pub fn page_hierarchy_display(&self) -> Option<String> {
        self.is_page_backed().then(|| {
            self.segments
                .iter()
                .map(PageName::as_str)
                .collect::<Vec<_>>()
                .join("/")
        })
    }

    pub fn hierarchy_display(&self) -> String {
        self.page_hierarchy_display()
            .expect("hierarchy_display is only valid for page-backed page ids")
    }

    pub fn canonical_identity_display(&self) -> String {
        match &self.location {
            PageLocation::Pages => format!("pages:{}", self.hierarchy_display()),
            PageLocation::Stream { stream_name } => {
                format!(
                    "stream:{}/{}",
                    stream_name.as_str(),
                    self.leaf_name().as_str()
                )
            }
        }
    }
}

impl PageLocation {
    pub fn workspace_path_for_page_id(&self, page_id: &PageId) -> Result<PathBuf, PagePathError> {
        if page_id.location() != self {
            return Err(PagePathError::NestedPath);
        }

        match self {
            Self::Pages => Ok(PathBuf::from(PAGES_ROOT).join(flat_page_file_name(page_id))),
            Self::Stream { stream_name } => {
                Ok(PathBuf::from(stream_name.as_str())
                    .join(markdown_file_name(page_id.leaf_name())))
            }
        }
    }

    pub fn parent_page_id(&self, page_id: &PageId) -> Option<PageId> {
        match self {
            Self::Pages => page_id.parent(),
            Self::Stream { .. } => None,
        }
    }

    pub fn ancestor_page_ids(&self, page_id: &PageId) -> Vec<PageId> {
        match self {
            Self::Pages => page_id.ancestors(),
            Self::Stream { .. } => Vec::new(),
        }
    }

    pub fn is_page_backed(&self) -> bool {
        matches!(self, Self::Pages)
    }

    pub fn is_stream_backed(&self) -> bool {
        matches!(self, Self::Stream { .. })
    }
}

pub fn resolve_workspace_path(
    path: impl AsRef<Path>,
) -> Result<ResolvedWorkspacePath, PagePathError> {
    let path = path.as_ref();

    let components = normalized_components(path)?;
    if components.is_empty() {
        return Err(PagePathError::MissingFileName);
    }

    match components[0].as_str() {
        PAGES_ROOT => resolve_page_backed_workspace_path(&components),
        root_name if !is_reserved_root_name(root_name) => {
            resolve_stream_workspace_path(&components)
        }
        _ => Err(PagePathError::NestedPath),
    }
}

pub fn supported_workspace_markdown_path(
    path: impl AsRef<Path>,
) -> Result<Option<ResolvedWorkspacePath>, PagePathError> {
    let path = path.as_ref();
    let components = normalized_components(path)?;
    if components.is_empty() {
        return Err(PagePathError::MissingFileName);
    }

    match components[0].as_str() {
        PAGES_ROOT => resolve_workspace_path(path).map(Some),
        root_name if !is_reserved_root_name(root_name) => {
            if components.len() == 2 && is_stream_date_markdown_name(&components[1]) {
                resolve_workspace_path(path).map(Some)
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_identity_display())
    }
}

impl FromStr for PageId {
    type Err = PagePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.split('/'))
    }
}

fn validate_page_name(value: &str) -> Result<(), NameError> {
    if value.is_empty() {
        return Err(NameError::Empty);
    }

    if value == "." || value == ".." {
        return Err(NameError::DotSegment);
    }

    if value.contains(HIERARCHY_DELIMITER) {
        return Err(NameError::ContainsHierarchyDelimiter);
    }

    if value.to_ascii_lowercase().ends_with(MARKDOWN_EXTENSION) {
        return Err(NameError::ContainsMarkdownExtension);
    }

    for ch in value.chars() {
        if ch == '/' || ch == '\\' {
            return Err(NameError::ContainsPathSeparator);
        }

        if matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            return Err(NameError::ContainsReservedCharacter(ch));
        }

        if ch.is_control() {
            return Err(NameError::ContainsControlCharacter(ch));
        }
    }

    if is_windows_device_name(value) {
        return Err(NameError::ReservedWindowsDeviceName);
    }

    Ok(())
}

fn normalized_components(path: &Path) -> Result<Vec<String>, PagePathError> {
    if path.is_absolute() {
        return Err(PagePathError::AbsolutePath);
    }

    path.components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or(PagePathError::MissingFileName),
            Component::ParentDir => Err(PagePathError::ParentComponent),
            Component::CurDir => Err(PagePathError::NestedPath),
            _ => Err(PagePathError::NestedPath),
        })
        .collect()
}

fn resolve_page_backed_workspace_path(
    components: &[String],
) -> Result<ResolvedWorkspacePath, PagePathError> {
    if components.len() != 2 {
        return Err(PagePathError::NestedPath);
    }

    let page_id = parse_page_file_name(&components[1])?;
    Ok(ResolvedWorkspacePath {
        location: page_id.location().clone(),
        page_id,
    })
}

fn resolve_stream_workspace_path(
    components: &[String],
) -> Result<ResolvedWorkspacePath, PagePathError> {
    if components.len() != 2 {
        return Err(PagePathError::NestedPath);
    }

    let stream_name = PageName::new(components[0].clone()).map_err(PagePathError::from)?;
    let date_name = parse_stream_date_file_name(&components[1])?;
    let location = PageLocation::Stream {
        stream_name: stream_name.clone(),
    };
    let page_id =
        PageId::from_page_names_in_location(location.clone(), vec![stream_name, date_name])?;
    Ok(ResolvedWorkspacePath { page_id, location })
}

fn parse_page_file_name(file_name: &str) -> Result<PageId, PagePathError> {
    let stem =
        strip_markdown_extension(file_name).ok_or(PagePathError::MissingMarkdownExtension)?;
    if stem.is_empty() || stem.split(HIERARCHY_DELIMITER).any(str::is_empty) {
        return Err(PagePathError::EmptyHierarchySegment);
    }

    PageId::new(stem.split(HIERARCHY_DELIMITER))
}

fn parse_single_page_name_file_name(file_name: &str) -> Result<PageName, PagePathError> {
    let stem =
        strip_markdown_extension(file_name).ok_or(PagePathError::MissingMarkdownExtension)?;
    PageName::new(stem).map_err(PagePathError::from)
}

fn parse_stream_date_file_name(file_name: &str) -> Result<PageName, PagePathError> {
    if !is_stream_date_markdown_name(file_name) {
        return Err(PagePathError::NestedPath);
    }
    parse_single_page_name_file_name(file_name)
}

fn flat_page_file_name(page_id: &PageId) -> String {
    let file_stem = page_id
        .segments
        .iter()
        .map(PageName::as_str)
        .collect::<Vec<_>>()
        .join(HIERARCHY_DELIMITER);

    markdown_file_name_str(&file_stem)
}

fn markdown_file_name(page_name: &PageName) -> String {
    markdown_file_name_str(page_name.as_str())
}

fn markdown_file_name_str(stem: &str) -> String {
    format!("{stem}{MARKDOWN_EXTENSION}")
}

fn strip_markdown_extension(file_name: &str) -> Option<&str> {
    file_name
        .get(file_name.len().checked_sub(MARKDOWN_EXTENSION.len())?..)
        .and_then(|suffix| {
            suffix
                .eq_ignore_ascii_case(MARKDOWN_EXTENSION)
                .then(|| &file_name[..file_name.len() - MARKDOWN_EXTENSION.len()])
        })
}

fn is_windows_device_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number))
        || upper
            .strip_prefix("LPT")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_id_round_trips_to_flat_workspace_path() {
        let page_id = PageId::new(["A", "B", "C"]).unwrap();

        assert_eq!(
            page_id.to_workspace_path(),
            PathBuf::from("pages").join("A___B___C.md")
        );
        assert_eq!(
            PageId::from_workspace_path("pages/A___B___C.md").unwrap(),
            page_id
        );
        assert_eq!(page_id.hierarchy_display(), "A/B/C");
        assert!(page_id.is_page_backed());
    }

    #[test]
    fn stream_paths_resolve_to_distinct_page_ids_and_locations() {
        let journal = resolve_workspace_path("journal/2026_05_07.md").unwrap();
        let diary = resolve_workspace_path("diary/2026_05_07.md").unwrap();

        assert_eq!(
            journal.page_id.canonical_identity_display(),
            "stream:journal/2026_05_07"
        );
        assert_eq!(
            diary.page_id.canonical_identity_display(),
            "stream:diary/2026_05_07"
        );
        assert_ne!(journal.page_id, diary.page_id);
        assert_eq!(
            journal
                .location
                .workspace_path_for_page_id(&journal.page_id)
                .unwrap(),
            PathBuf::from("journal").join("2026_05_07.md")
        );
        assert_eq!(journal.location.parent_page_id(&journal.page_id), None);
        assert!(journal.page_id.is_stream_backed());
    }

    #[test]
    fn page_and_stream_with_same_segments_are_distinct() {
        let page = PageId::new(["journal", "2026_05_07"]).unwrap();
        let stream = resolve_workspace_path("journal/2026_05_07.md")
            .unwrap()
            .page_id;

        assert_ne!(page, stream);
        assert_eq!(page.hierarchy_display(), "journal/2026_05_07");
        assert_eq!(stream.page_hierarchy_display(), None);
    }

    #[test]
    fn page_id_exposes_parent_and_ancestors() {
        let page_id = PageId::new(["A", "B", "C"]).unwrap();

        assert_eq!(page_id.parent().unwrap().hierarchy_display(), "A/B");
        assert_eq!(
            page_id
                .ancestors()
                .iter()
                .map(PageId::hierarchy_display)
                .collect::<Vec<_>>(),
            vec!["A", "A/B"]
        );
    }

    #[test]
    fn stream_pages_have_no_hierarchy_relationships() {
        let page_id = resolve_workspace_path("journal/2026_05_07.md")
            .unwrap()
            .page_id;

        assert!(page_id.parent().is_none());
        assert!(page_id.ancestors().is_empty());
        assert!(page_id.page_hierarchy_display().is_none());
    }

    #[test]
    fn rejects_invalid_page_names() {
        assert_eq!(PageName::new("").unwrap_err(), NameError::Empty);
        assert_eq!(PageName::new(".").unwrap_err(), NameError::DotSegment);
        assert_eq!(
            PageName::new("A___B").unwrap_err(),
            NameError::ContainsHierarchyDelimiter
        );
        assert_eq!(
            PageName::new("A.md").unwrap_err(),
            NameError::ContainsMarkdownExtension
        );
        assert_eq!(
            PageName::new("A/B").unwrap_err(),
            NameError::ContainsPathSeparator
        );
        assert_eq!(
            PageName::new("A:B").unwrap_err(),
            NameError::ContainsReservedCharacter(':')
        );
        assert_eq!(
            PageName::new("CON").unwrap_err(),
            NameError::ReservedWindowsDeviceName
        );
    }

    #[test]
    fn rejects_non_flat_or_non_markdown_paths() {
        assert_eq!(
            PageId::from_workspace_path("pages/A/B.md").unwrap_err(),
            PagePathError::NestedPath
        );
        assert_eq!(
            PageId::from_workspace_path("../pages/A.md").unwrap_err(),
            PagePathError::ParentComponent
        );
        assert_eq!(
            PageId::from_workspace_path("pages/A.txt").unwrap_err(),
            PagePathError::MissingMarkdownExtension
        );
        assert_eq!(
            PageId::from_workspace_path("pages/A______B.md").unwrap_err(),
            PagePathError::EmptyHierarchySegment
        );
    }

    #[test]
    fn accepts_mixed_case_markdown_extensions() {
        assert_eq!(
            PageId::from_workspace_path("pages/A___B.Md")
                .unwrap()
                .hierarchy_display(),
            "A/B"
        );
    }

    #[test]
    fn ignores_markdown_outside_supported_roots() {
        assert!(supported_workspace_markdown_path("A.md").unwrap().is_none());
        assert!(
            supported_workspace_markdown_path("archive/Old.md")
                .unwrap()
                .is_none()
        );
        assert!(
            supported_workspace_markdown_path("archive/2026_05_07.md")
                .unwrap()
                .is_some()
        );
    }
}

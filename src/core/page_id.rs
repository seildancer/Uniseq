use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

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
pub struct PageId {
    segments: Vec<PageName>,
}

impl PageId {
    pub fn new<I, S>(segments: I) -> Result<Self, PagePathError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let segments = segments
            .into_iter()
            .map(|segment| PageName::new(segment.as_ref()).map_err(PagePathError::from))
            .collect::<Result<Vec<_>, _>>()?;

        Self::from_page_names(segments)
    }

    pub fn from_page_names(segments: Vec<PageName>) -> Result<Self, PagePathError> {
        if segments.is_empty() {
            return Err(PagePathError::EmptyPageId);
        }

        Ok(Self { segments })
    }

    pub fn from_workspace_path(path: impl AsRef<Path>) -> Result<Self, PagePathError> {
        let path = path.as_ref();

        if path.is_absolute() {
            return Err(PagePathError::AbsolutePath);
        }

        let mut components = path.components();
        let file_name = match components.next() {
            Some(Component::Normal(file_name)) => file_name,
            Some(Component::ParentDir) => return Err(PagePathError::ParentComponent),
            Some(_) => return Err(PagePathError::NestedPath),
            None => return Err(PagePathError::MissingFileName),
        };

        if components.next().is_some() {
            return Err(PagePathError::NestedPath);
        }

        let file_name = file_name.to_str().ok_or(PagePathError::MissingFileName)?;

        let stem = file_name
            .strip_suffix(MARKDOWN_EXTENSION)
            .or_else(|| file_name.strip_suffix(".MD"))
            .ok_or(PagePathError::MissingMarkdownExtension)?;

        if stem.is_empty() || stem.split(HIERARCHY_DELIMITER).any(str::is_empty) {
            return Err(PagePathError::EmptyHierarchySegment);
        }

        Self::new(stem.split(HIERARCHY_DELIMITER))
    }

    pub fn to_workspace_path(&self) -> PathBuf {
        let file_stem = self
            .segments
            .iter()
            .map(PageName::as_str)
            .collect::<Vec<_>>()
            .join(HIERARCHY_DELIMITER);

        PathBuf::from(format!("{file_stem}{MARKDOWN_EXTENSION}"))
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
        let parent_len = self.segments.len().checked_sub(1)?;
        if parent_len == 0 {
            return None;
        }

        Some(Self {
            segments: self.segments[..parent_len].to_vec(),
        })
    }

    pub fn ancestors(&self) -> Vec<Self> {
        (1..self.segments.len())
            .map(|len| Self {
                segments: self.segments[..len].to_vec(),
            })
            .collect()
    }

    pub fn hierarchy_display(&self) -> String {
        self.segments
            .iter()
            .map(PageName::as_str)
            .collect::<Vec<_>>()
            .join("/")
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hierarchy_display())
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

        assert_eq!(page_id.to_workspace_path(), PathBuf::from("A___B___C.md"));
        assert_eq!(
            PageId::from_workspace_path("A___B___C.md").unwrap(),
            page_id
        );
        assert_eq!(page_id.hierarchy_display(), "A/B/C");
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
    fn root_pages_have_no_parent() {
        let page_id = PageId::new(["A"]).unwrap();

        assert!(page_id.parent().is_none());
        assert!(page_id.ancestors().is_empty());
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
            PageId::from_workspace_path("A/B.md").unwrap_err(),
            PagePathError::NestedPath
        );
        assert_eq!(
            PageId::from_workspace_path("../A.md").unwrap_err(),
            PagePathError::ParentComponent
        );
        assert_eq!(
            PageId::from_workspace_path("A.txt").unwrap_err(),
            PagePathError::MissingMarkdownExtension
        );
        assert_eq!(
            PageId::from_workspace_path("A______B.md").unwrap_err(),
            PagePathError::EmptyHierarchySegment
        );
    }
}

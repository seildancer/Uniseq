use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    Empty,
    DotSegment,
    ContainsHierarchyDelimiter,
    ContainsMarkdownExtension,
    ContainsPathSeparator,
    ContainsReservedCharacter(char),
    ContainsControlCharacter(char),
    ReservedWindowsDeviceName,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "page name segment cannot be empty"),
            Self::DotSegment => write!(f, "page name segment cannot be '.' or '..'"),
            Self::ContainsHierarchyDelimiter => {
                write!(f, "page name segment cannot contain '___'")
            }
            Self::ContainsMarkdownExtension => {
                write!(f, "page name segment cannot contain a .md suffix")
            }
            Self::ContainsPathSeparator => {
                write!(f, "page name segment cannot contain path separators")
            }
            Self::ContainsReservedCharacter(ch) => {
                write!(f, "page name segment contains reserved character '{ch}'")
            }
            Self::ContainsControlCharacter(ch) => {
                write!(
                    f,
                    "page name segment contains control character U+{:04X}",
                    *ch as u32
                )
            }
            Self::ReservedWindowsDeviceName => {
                write!(
                    f,
                    "page name segment cannot be a reserved Windows device name"
                )
            }
        }
    }
}

impl std::error::Error for NameError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagePathError {
    EmptyPageId,
    AbsolutePath,
    ParentComponent,
    NestedPath,
    MissingFileName,
    MissingMarkdownExtension,
    EmptyHierarchySegment,
    InvalidName(NameError),
}

impl fmt::Display for PagePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPageId => write!(f, "page id must contain at least one segment"),
            Self::AbsolutePath => write!(f, "page path must be workspace-relative"),
            Self::ParentComponent => write!(f, "page path cannot contain parent components"),
            Self::NestedPath => write!(f, "page path must use flat A___B.md hierarchy files"),
            Self::MissingFileName => write!(f, "page path must include a file name"),
            Self::MissingMarkdownExtension => write!(f, "page path must end in .md"),
            Self::EmptyHierarchySegment => {
                write!(f, "page path contains an empty hierarchy segment")
            }
            Self::InvalidName(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PagePathError {}

impl From<NameError> for PagePathError {
    fn from(value: NameError) -> Self {
        Self::InvalidName(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanError {
    EndBeforeStart { start: usize, end: usize },
    OutOfBounds { span_end: usize, text_len: usize },
    NotUtf8Boundary { offset: usize },
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndBeforeStart { start, end } => {
                write!(f, "span end {end} is before start {start}")
            }
            Self::OutOfBounds { span_end, text_len } => {
                write!(f, "span end {span_end} exceeds text length {text_len}")
            }
            Self::NotUtf8Boundary { offset } => {
                write!(f, "span offset {offset} is not a UTF-8 boundary")
            }
        }
    }
}

impl std::error::Error for SpanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserError {
    InvalidFenceMarker,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFenceMarker => write!(f, "invalid fenced code marker"),
        }
    }
}

impl std::error::Error for ParserError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    InvalidName(NameError),
    InvalidPagePath(PagePathError),
    InvalidSpan(SpanError),
    InvalidParse(ParserError),
    DuplicatePageIdentity { page_id: String },
    Io { path: PathBuf, kind: io::ErrorKind },
    StructuralConflict { path: PathBuf },
    MissingPage,
    MissingDestinationParent,
    DestinationPageExists,
    InvalidPageMove,
    UnsupportedStreamOperation { operation: &'static str },
    CorruptTransaction,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(err) => write!(f, "{err}"),
            Self::InvalidPagePath(err) => write!(f, "{err}"),
            Self::InvalidSpan(err) => write!(f, "{err}"),
            Self::InvalidParse(err) => write!(f, "{err}"),
            Self::DuplicatePageIdentity { page_id } => {
                write!(f, "duplicate page identity detected for '{page_id}'")
            }
            Self::Io { path, kind } => {
                write!(f, "i/o error at '{}': {kind}", path.display())
            }
            Self::StructuralConflict { path } => {
                write!(
                    f,
                    "structural operation aborted because '{}' changed on disk",
                    path.display()
                )
            }
            Self::MissingPage => write!(f, "page does not exist in cache"),
            Self::MissingDestinationParent => {
                write!(f, "destination parent page does not exist")
            }
            Self::DestinationPageExists => write!(f, "destination page already exists"),
            Self::InvalidPageMove => write!(f, "page move would create an invalid hierarchy"),
            Self::UnsupportedStreamOperation { operation } => {
                write!(f, "stream pages do not support the '{operation}' operation")
            }
            Self::CorruptTransaction => write!(f, "transaction record is missing or invalid"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<NameError> for CoreError {
    fn from(value: NameError) -> Self {
        Self::InvalidName(value)
    }
}

impl From<PagePathError> for CoreError {
    fn from(value: PagePathError) -> Self {
        Self::InvalidPagePath(value)
    }
}

impl From<SpanError> for CoreError {
    fn from(value: SpanError) -> Self {
        Self::InvalidSpan(value)
    }
}

impl From<ParserError> for CoreError {
    fn from(value: ParserError) -> Self {
        Self::InvalidParse(value)
    }
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, error: &io::Error) -> Self {
        Self::Io {
            path: path.into(),
            kind: error.kind(),
        }
    }
}

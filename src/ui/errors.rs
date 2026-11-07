//! Errors for loading UI templates.
//!
//! The small value parsers (colors, lengths, selectors, single declarations) work on
//! snippets and report plain `String` messages. The document parsers (markup and
//! stylesheet) know where each snippet sits in the file, so they wrap those messages in a
//! `ParseError` with a line and column. That's what an author needs to find the mistake,
//! especially when a hot reload is rejected.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Markup,
    Stylesheet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: SourceKind,
    /// The file the source came from, when it was loaded from disk.
    pub path: Option<PathBuf>,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub column: usize,
    pub message: String,
}

impl ParseError {
    /// Creates an error pointing at `byte_offset` in `src`.
    pub fn at(kind: SourceKind, src: &str, byte_offset: usize, message: impl Into<String>) -> Self {
        let (line, column) = line_and_column(src, byte_offset);
        ParseError {
            kind,
            path: None,
            line,
            column,
            message: message.into(),
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}", path.display())?,
            None => match self.kind {
                SourceKind::Markup => write!(f, "<markup>")?,
                SourceKind::Stylesheet => write!(f, "<stylesheet>")?,
            },
        }
        write!(f, ":{}:{}: {}", self.line, self.column, self.message)
    }
}

impl Error for ParseError {}

/// Converts a byte offset into a 1-based (line, column) pair. Offsets past the end or in
/// the middle of a character are clamped to the nearest valid position before them.
fn line_and_column(src: &str, byte_offset: usize) -> (usize, usize) {
    let mut offset = byte_offset.min(src.len());
    while !src.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let column = before[line_start..].chars().count() + 1;
    (line, column)
}

/// Errors from `UiAssetStore`.
#[derive(Debug)]
pub enum UiAssetError {
    /// A template file couldn't be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A template's markup or stylesheet is invalid.
    Parse(ParseError),
    /// A font couldn't be parsed. fontdue reports errors as static strings.
    Font { name: String, message: &'static str },
}

impl fmt::Display for UiAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiAssetError::Io { path, .. } => write!(f, "failed to read {}", path.display()),
            UiAssetError::Parse(error) => write!(f, "{error}"),
            UiAssetError::Font { name, message } => {
                write!(f, "failed to parse font '{name}': {message}")
            }
        }
    }
}

impl Error for UiAssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            UiAssetError::Io { source, .. } => Some(source),
            UiAssetError::Parse(error) => Some(error),
            UiAssetError::Font { .. } => None,
        }
    }
}

impl From<ParseError> for UiAssetError {
    fn from(error: ParseError) -> Self {
        UiAssetError::Parse(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_column_are_one_based() {
        let src = "ab\ncd\nef";
        assert_eq!(line_and_column(src, 0), (1, 1));
        assert_eq!(line_and_column(src, 1), (1, 2));
        assert_eq!(line_and_column(src, 3), (2, 1));
        assert_eq!(line_and_column(src, 7), (3, 2));
    }

    #[test]
    fn line_and_column_clamps_out_of_range_offsets() {
        assert_eq!(line_and_column("ab", 100), (1, 3));
        // offset 1 is inside the two-byte 'é'
        assert_eq!(line_and_column("é", 1), (1, 1));
    }

    #[test]
    fn display_prefers_path_over_kind() {
        let error = ParseError::at(SourceKind::Stylesheet, "a\nb", 2, "bad");
        assert_eq!(error.to_string(), "<stylesheet>:2:1: bad");
        let error = error.with_path("ui/panel.css");
        assert_eq!(error.to_string(), "ui/panel.css:2:1: bad");
    }
}

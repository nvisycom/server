//! Filtering options for document file queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// File format categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    /// PDF documents (.pdf).
    Pdf,
    /// Microsoft Word documents (.doc, .docx).
    Doc,
    /// Plain text files (.txt).
    Txt,
    /// Markdown files (.md).
    Md,
    /// CSV files (.csv).
    Csv,
    /// JSON files (.json).
    Json,
    /// PNG images (.png).
    Png,
    /// JPEG images (.jpg, .jpeg).
    Jpeg,
}

impl FileFormat {
    /// Returns the file extensions associated with this format.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Pdf => &["pdf"],
            Self::Doc => &["doc", "docx"],
            Self::Txt => &["txt"],
            Self::Md => &["md", "markdown"],
            Self::Csv => &["csv"],
            Self::Json => &["json"],
            Self::Png => &["png"],
            Self::Jpeg => &["jpg", "jpeg"],
        }
    }

    /// Returns the MIME types associated with this format.
    pub fn mime_types(&self) -> &'static [&'static str] {
        match self {
            Self::Pdf => &["application/pdf"],
            Self::Doc => &[
                "application/msword",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ],
            Self::Txt => &["text/plain"],
            Self::Md => &["text/markdown", "text/x-markdown"],
            Self::Csv => &["text/csv"],
            Self::Json => &["application/json"],
            Self::Png => &["image/png"],
            Self::Jpeg => &["image/jpeg"],
        }
    }
}

/// Filter options for document files.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FileFilter {
    /// Filter by file formats (any match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<FileFormat>>,
}

impl FileFilter {
    /// Creates a new empty filter.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by file formats.
    #[inline]
    pub fn with_formats(mut self, formats: Vec<FileFormat>) -> Self {
        self.formats = Some(formats);
        self
    }

    /// Returns whether any filter is active.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.formats.as_ref().is_none_or(|f| f.is_empty())
    }

    /// Returns all MIME types from the format filters.
    pub fn mime_types(&self) -> Vec<&'static str> {
        self.formats
            .as_ref()
            .map(|formats| {
                formats
                    .iter()
                    .flat_map(|f| f.mime_types().iter().copied())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns all file extensions from the format filters.
    pub fn extensions(&self) -> Vec<&'static str> {
        self.formats
            .as_ref()
            .map(|formats| {
                formats
                    .iter()
                    .flat_map(|f| f.extensions().iter().copied())
                    .collect()
            })
            .unwrap_or_default()
    }
}

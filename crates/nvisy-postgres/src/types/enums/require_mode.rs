//! Require mode enumeration for file content type classification.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Classifies the content type of uploaded files.
///
/// This enumeration corresponds to the `REQUIRE_MODE` PostgreSQL enum and is used
/// to categorize files based on their content type for appropriate processing.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::RequireMode"]
pub enum RequireMode {
    /// Unknown or unrecognized file type.
    #[db_rename = "unknown"]
    #[serde(rename = "unknown")]
    #[default]
    Unknown,

    /// Text documents (PDF, DOCX, TXT, etc.).
    #[db_rename = "document"]
    #[serde(rename = "document")]
    Document,

    /// Image files (PNG, JPG, SVG, etc.).
    #[db_rename = "image"]
    #[serde(rename = "image")]
    Image,

    /// Spreadsheet files (XLSX, CSV, etc.).
    #[db_rename = "spreadsheet"]
    #[serde(rename = "spreadsheet")]
    Spreadsheet,

    /// Presentation files (PPTX, KEY, etc.).
    #[db_rename = "presentation"]
    #[serde(rename = "presentation")]
    Presentation,

    /// Audio files (MP3, WAV, etc.).
    #[db_rename = "audio"]
    #[serde(rename = "audio")]
    Audio,

    /// Video files (MP4, MOV, etc.).
    #[db_rename = "video"]
    #[serde(rename = "video")]
    Video,

    /// Archive files (ZIP, TAR, etc.).
    #[db_rename = "archive"]
    #[serde(rename = "archive")]
    Archive,

    /// Data files (JSON, XML, CSV, etc.).
    #[db_rename = "data"]
    #[serde(rename = "data")]
    Data,
}

impl RequireMode {
    /// Returns whether this is a text-based content type.
    #[inline]
    pub fn is_text_based(self) -> bool {
        matches!(
            self,
            RequireMode::Document | RequireMode::Spreadsheet | RequireMode::Data
        )
    }

    /// Returns whether this is a visual content type.
    #[inline]
    pub fn is_visual(self) -> bool {
        matches!(
            self,
            RequireMode::Image | RequireMode::Video | RequireMode::Presentation
        )
    }

    /// Returns whether this is a media content type.
    #[inline]
    pub fn is_media(self) -> bool {
        matches!(
            self,
            RequireMode::Image | RequireMode::Audio | RequireMode::Video
        )
    }

    /// Returns whether this content type can be indexed for search.
    #[inline]
    pub fn is_indexable(self) -> bool {
        matches!(
            self,
            RequireMode::Document
                | RequireMode::Spreadsheet
                | RequireMode::Presentation
                | RequireMode::Data
        )
    }

    /// Returns whether this content type requires extraction before processing.
    #[inline]
    pub fn requires_extraction(self) -> bool {
        matches!(self, RequireMode::Archive)
    }

    /// Returns whether this content type requires transcription.
    #[inline]
    pub fn requires_transcription(self) -> bool {
        matches!(self, RequireMode::Audio | RequireMode::Video)
    }

    /// Returns whether this content type requires OCR.
    #[inline]
    pub fn requires_ocr(self) -> bool {
        matches!(self, RequireMode::Image)
    }
}

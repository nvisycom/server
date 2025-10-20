//! File type enumeration for file categorization and processing.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the high-level category of a file for processing and handling.
///
/// This enumeration corresponds to the `FILE_TYPE` PostgreSQL enum and is used
/// to categorize uploaded files by their general type for appropriate processing
/// and handling within the document management system.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::FileType"]
pub enum FileType {
    /// Text documents (PDF, DOC, DOCX, TXT, RTF, etc.)
    #[db_rename = "document"]
    #[serde(rename = "document")]
    #[default]
    Document,

    /// Images (PNG, JPG, JPEG, GIF, BMP, SVG, TIFF, etc.)
    #[db_rename = "image"]
    #[serde(rename = "image")]
    Image,

    /// Video files (MP4, AVI, MOV, WMV, MKV, FLV, etc.)
    #[db_rename = "video"]
    #[serde(rename = "video")]
    Video,

    /// Audio files (MP3, WAV, FLAC, AAC, OGG, M4A, etc.)
    #[db_rename = "audio"]
    #[serde(rename = "audio")]
    Audio,

    /// Compressed archives (ZIP, RAR, 7Z, TAR, GZ, etc.)
    #[db_rename = "archive"]
    #[serde(rename = "archive")]
    Archive,

    /// Data files (CSV, JSON, XML, YAML, SQL, etc.)
    #[db_rename = "data"]
    #[serde(rename = "data")]
    Data,

    /// Source code files (JS, TS, RS, PY, JAVA, C, CPP, etc.)
    #[db_rename = "code"]
    #[serde(rename = "code")]
    Code,
}

impl FileType {
    /// Returns whether this file type requires text extraction for processing.
    #[inline]
    pub fn requires_text_extraction(self) -> bool {
        matches!(self, FileType::Document | FileType::Code | FileType::Data)
    }

    /// Returns whether this file type requires OCR (Optical Character Recognition).
    #[inline]
    pub fn may_require_ocr(self) -> bool {
        matches!(self, FileType::Image | FileType::Document)
    }

    /// Returns whether this file type requires transcription (audio/video to text).
    #[inline]
    pub fn may_require_transcription(self) -> bool {
        matches!(self, FileType::Audio | FileType::Video)
    }

    /// Returns whether this file type can contain searchable text content.
    #[inline]
    pub fn has_searchable_content(self) -> bool {
        matches!(self, FileType::Document | FileType::Code | FileType::Data)
    }

    /// Returns whether this file type typically requires special processing.
    #[inline]
    pub fn requires_special_processing(self) -> bool {
        matches!(self, FileType::Archive | FileType::Video | FileType::Audio)
    }

    /// Returns whether this file type is considered media content.
    #[inline]
    pub fn is_media(self) -> bool {
        matches!(self, FileType::Image | FileType::Video | FileType::Audio)
    }

    /// Returns whether this file type is textual in nature.
    #[inline]
    pub fn is_textual(self) -> bool {
        matches!(self, FileType::Document | FileType::Code | FileType::Data)
    }

    /// Returns whether this file type supports metadata extraction.
    #[inline]
    pub fn supports_metadata_extraction(self) -> bool {
        // All file types support some level of metadata extraction
        true
    }

    /// Returns the processing priority for this file type (1 = highest, 10 = lowest).
    #[inline]
    pub fn processing_priority(self) -> u8 {
        match self {
            // High priority - commonly processed, quick to handle
            FileType::Document | FileType::Data | FileType::Code => 2,
            // Medium priority - may require more processing time
            FileType::Image => 4,
            // Lower priority - resource intensive
            FileType::Audio => 6,
            FileType::Video => 7,
            // Lowest priority - needs extraction first
            FileType::Archive => 8,
        }
    }

    /// Returns common file extensions associated with this file type.
    pub fn common_extensions(self) -> &'static [&'static str] {
        match self {
            FileType::Document => &[
                "pdf", "doc", "docx", "txt", "rtf", "odt", "pages", "md", "tex",
            ],
            FileType::Image => &[
                "jpg", "jpeg", "png", "gif", "bmp", "svg", "tiff", "tif", "webp", "ico",
            ],
            FileType::Video => &[
                "mp4", "avi", "mov", "wmv", "flv", "webm", "mkv", "m4v", "3gp", "ogv",
            ],
            FileType::Audio => &[
                "mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "aiff", "au",
            ],
            FileType::Archive => &[
                "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "cab", "arj", "lz",
            ],
            FileType::Data => &[
                "csv", "json", "xml", "yaml", "yml", "sql", "db", "tsv", "parquet", "avro",
            ],
            FileType::Code => &[
                "js", "ts", "rs", "py", "java", "c", "cpp", "h", "hpp", "cs", "php", "rb", "go",
                "kt", "swift", "scala", "clj", "hs", "elm", "dart", "lua", "r",
            ],
        }
    }

    /// Returns a description of what files this type includes.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            FileType::Document => "Text documents, PDFs, and office files",
            FileType::Image => "Photos, graphics, and image files",
            FileType::Video => "Video files and multimedia content",
            FileType::Audio => "Audio files and sound recordings",
            FileType::Archive => "Compressed archives and zip files",
            FileType::Data => "Structured data files and databases",
            FileType::Code => "Source code and programming files",
        }
    }

    /// Attempts to determine file type from a file extension.
    pub fn from_extension(extension: &str) -> Option<FileType> {
        let ext = extension.to_lowercase();
        let ext = ext.trim_start_matches('.');

        Self::iter().find(|&file_type| file_type.common_extensions().contains(&ext))
    }

    /// Attempts to determine file type from a MIME type.
    pub fn from_mime_type(mime_type: &str) -> Option<FileType> {
        let mime = mime_type.to_lowercase();

        if mime.starts_with("text/") || mime.contains("document") || mime.contains("pdf") {
            Some(FileType::Document)
        } else if mime.starts_with("image/") {
            Some(FileType::Image)
        } else if mime.starts_with("video/") {
            Some(FileType::Video)
        } else if mime.starts_with("audio/") {
            Some(FileType::Audio)
        } else if mime.contains("zip") || mime.contains("compressed") || mime.contains("archive") {
            Some(FileType::Archive)
        } else if mime.contains("json") || mime.contains("csv") || mime.contains("xml") {
            Some(FileType::Data)
        } else if mime.contains("javascript")
            || mime.contains("x-python")
            || mime.contains("x-rust")
        {
            Some(FileType::Code)
        } else {
            None
        }
    }

    /// Returns file types that support text-based processing.
    pub fn text_processable_types() -> &'static [FileType] {
        &[FileType::Document, FileType::Code, FileType::Data]
    }

    /// Returns file types that are considered multimedia.
    pub fn multimedia_types() -> &'static [FileType] {
        &[FileType::Image, FileType::Video, FileType::Audio]
    }
}

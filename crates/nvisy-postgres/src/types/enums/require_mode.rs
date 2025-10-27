//! Require mode enumeration for file processing requirements.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

use crate::types::FileType;

/// Defines the processing requirements for input files.
///
/// This enumeration corresponds to the `REQUIRE_MODE` PostgreSQL enum and is used
/// to specify what type of processing is needed to extract content from uploaded files.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::RequireMode"]
pub enum RequireMode {
    /// Plain text content that is ready for analysis without additional processing
    #[db_rename = "text"]
    #[serde(rename = "text")]
    #[default]
    Text,

    /// File requires Optical Character Recognition (OCR) to extract text
    #[db_rename = "ocr"]
    #[serde(rename = "ocr")]
    Ocr,

    /// File requires audio/video transcription to convert speech to text
    #[db_rename = "transcribe"]
    #[serde(rename = "transcribe")]
    Transcribe,

    /// File may require multiple processing modes (combination of text, OCR, transcription)
    #[db_rename = "mixed"]
    #[serde(rename = "mixed")]
    Mixed,
}

impl RequireMode {
    /// Returns whether this mode requires text extraction processing.
    #[inline]
    pub fn requires_text_extraction(self) -> bool {
        matches!(self, RequireMode::Text | RequireMode::Mixed)
    }

    /// Returns whether this mode requires OCR processing.
    #[inline]
    pub fn requires_ocr(self) -> bool {
        matches!(self, RequireMode::Ocr | RequireMode::Mixed)
    }

    /// Returns whether this mode requires transcription processing.
    #[inline]
    pub fn requires_transcription(self) -> bool {
        matches!(self, RequireMode::Transcribe | RequireMode::Mixed)
    }

    /// Returns whether this mode requires any special processing.
    #[inline]
    pub fn requires_processing(self) -> bool {
        !matches!(self, RequireMode::Text)
    }

    /// Returns whether this mode involves multiple processing types.
    #[inline]
    pub fn is_complex(self) -> bool {
        matches!(self, RequireMode::Mixed)
    }

    /// Returns whether this mode is ready for immediate analysis.
    #[inline]
    pub fn is_ready_for_analysis(self) -> bool {
        matches!(self, RequireMode::Text)
    }

    /// Returns whether this mode requires external processing services.
    #[inline]
    pub fn requires_external_services(self) -> bool {
        matches!(
            self,
            RequireMode::Ocr | RequireMode::Transcribe | RequireMode::Mixed
        )
    }

    /// Returns whether this mode typically has higher processing costs.
    #[inline]
    pub fn is_expensive_to_process(self) -> bool {
        matches!(
            self,
            RequireMode::Ocr | RequireMode::Transcribe | RequireMode::Mixed
        )
    }

    /// Returns the estimated processing complexity (1 = simple, 5 = very complex).
    #[inline]
    pub fn processing_complexity(self) -> u8 {
        match self {
            RequireMode::Text => 1,
            RequireMode::Ocr => 3,
            RequireMode::Transcribe => 4,
            RequireMode::Mixed => 5,
        }
    }

    /// Returns the estimated processing time factor (multiplier for base time).
    #[inline]
    pub fn processing_time_factor(self) -> f32 {
        match self {
            RequireMode::Text => 1.0,
            RequireMode::Ocr => 3.0,
            RequireMode::Transcribe => 5.0,
            RequireMode::Mixed => 8.0,
        }
    }

    /// Returns the types of processing that this mode typically involves.
    pub fn processing_types(self) -> &'static [&'static str] {
        match self {
            RequireMode::Text => &["text_extraction"],
            RequireMode::Ocr => &["optical_character_recognition", "image_processing"],
            RequireMode::Transcribe => &["speech_recognition", "audio_processing"],
            RequireMode::Mixed => &[
                "text_extraction",
                "optical_character_recognition",
                "speech_recognition",
                "image_processing",
                "audio_processing",
            ],
        }
    }

    /// Determines the appropriate require mode based on file type and content.
    pub fn from_file_type_and_content(
        file_type: Option<FileType>,
        has_text: bool,
        has_images: bool,
        has_audio: bool,
    ) -> RequireMode {
        match file_type {
            Some(FileType::Document) if has_text && !has_images => RequireMode::Text,
            Some(FileType::Document) if has_images => RequireMode::Ocr,
            Some(FileType::Image) => RequireMode::Ocr,
            Some(FileType::Audio) => RequireMode::Transcribe,
            Some(FileType::Video) => RequireMode::Transcribe,
            Some(FileType::Code) | Some(FileType::Data) => RequireMode::Text,
            Some(FileType::Archive) => RequireMode::Mixed,
            _ => {
                // Determine based on content flags
                match (has_text, has_images, has_audio) {
                    (true, false, false) => RequireMode::Text,
                    (false, true, false) => RequireMode::Ocr,
                    (false, false, true) => RequireMode::Transcribe,
                    _ => RequireMode::Mixed,
                }
            }
        }
    }

    /// Returns require modes that need external processing.
    pub fn external_processing_modes() -> &'static [RequireMode] {
        &[
            RequireMode::Ocr,
            RequireMode::Transcribe,
            RequireMode::Mixed,
        ]
    }
}

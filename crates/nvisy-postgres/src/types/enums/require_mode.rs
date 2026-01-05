//! Require mode enumeration for file processing requirements.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the processing requirements for input files.
///
/// This enumeration corresponds to the `REQUIRE_MODE` PostgreSQL enum and is used
/// to specify what type of processing is needed to extract content from uploaded files.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::RequireMode"]
pub enum RequireMode {
    /// No special processing required.
    #[db_rename = "none"]
    #[serde(rename = "none")]
    #[default]
    None,

    /// Requires Optical Character Recognition (OCR).
    #[db_rename = "optical"]
    #[serde(rename = "optical")]
    Optical,

    /// Requires Vision Language Model (VLM).
    #[db_rename = "language"]
    #[serde(rename = "language")]
    Language,

    /// Requires both OCR and VLM processing.
    #[db_rename = "both"]
    #[serde(rename = "both")]
    Both,
}

impl RequireMode {
    /// Returns whether this mode requires OCR processing.
    #[inline]
    pub fn requires_ocr(self) -> bool {
        matches!(self, RequireMode::Optical | RequireMode::Both)
    }

    /// Returns whether this mode requires VLM processing.
    #[inline]
    pub fn requires_vlm(self) -> bool {
        matches!(self, RequireMode::Language | RequireMode::Both)
    }

    /// Returns whether this mode requires any special processing.
    #[inline]
    pub fn requires_processing(self) -> bool {
        !matches!(self, RequireMode::None)
    }

    /// Returns whether this mode involves multiple processing types.
    #[inline]
    pub fn is_complex(self) -> bool {
        matches!(self, RequireMode::Both)
    }

    /// Returns whether this mode is ready for immediate analysis.
    #[inline]
    pub fn is_ready_for_analysis(self) -> bool {
        matches!(self, RequireMode::None)
    }

    /// Returns whether this mode requires external processing services.
    #[inline]
    pub fn requires_external_services(self) -> bool {
        matches!(
            self,
            RequireMode::Optical | RequireMode::Language | RequireMode::Both
        )
    }

    /// Returns whether this mode typically has higher processing costs.
    #[inline]
    pub fn is_expensive_to_process(self) -> bool {
        matches!(
            self,
            RequireMode::Optical | RequireMode::Language | RequireMode::Both
        )
    }

    /// Returns the estimated processing complexity (1 = simple, 5 = very complex).
    #[inline]
    pub fn processing_complexity(self) -> u8 {
        match self {
            RequireMode::None => 1,
            RequireMode::Optical => 3,
            RequireMode::Language => 4,
            RequireMode::Both => 5,
        }
    }

    /// Returns the estimated processing time factor (multiplier for base time).
    #[inline]
    pub fn processing_time_factor(self) -> f32 {
        match self {
            RequireMode::None => 1.0,
            RequireMode::Optical => 3.0,
            RequireMode::Language => 5.0,
            RequireMode::Both => 8.0,
        }
    }

    /// Returns the types of processing that this mode typically involves.
    pub fn processing_types(self) -> &'static [&'static str] {
        match self {
            RequireMode::None => &[],
            RequireMode::Optical => &["optical_character_recognition"],
            RequireMode::Language => &["vision_language_model"],
            RequireMode::Both => &["optical_character_recognition", "vision_language_model"],
        }
    }

    /// Returns require modes that need external processing.
    pub fn external_processing_modes() -> &'static [RequireMode] {
        &[
            RequireMode::Optical,
            RequireMode::Language,
            RequireMode::Both,
        ]
    }
}

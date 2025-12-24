//! Content segmentation enumeration for knowledge extraction.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the content segmentation strategy for document processing.
///
/// This enumeration corresponds to the `CONTENT_SEGMENTATION` PostgreSQL enum and is used
/// to specify how document content should be segmented for knowledge extraction.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ContentSegmentation"]
pub enum ContentSegmentation {
    /// No segmentation applied - process content as a whole
    #[db_rename = "none"]
    #[serde(rename = "none")]
    None,

    /// Semantic-based segmentation - split by meaning and context
    #[db_rename = "semantic"]
    #[serde(rename = "semantic")]
    #[default]
    Semantic,

    /// Fixed-size segmentation - split by character or token count
    #[db_rename = "fixed"]
    #[serde(rename = "fixed")]
    Fixed,
}

impl ContentSegmentation {
    /// Returns whether this strategy uses semantic analysis.
    #[inline]
    pub fn uses_semantic_analysis(self) -> bool {
        matches!(self, ContentSegmentation::Semantic)
    }

    /// Returns whether this strategy uses fixed-size chunks.
    #[inline]
    pub fn uses_fixed_chunks(self) -> bool {
        matches!(self, ContentSegmentation::Fixed)
    }

    /// Returns whether segmentation is disabled.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, ContentSegmentation::None)
    }

    /// Returns whether this strategy requires advanced NLP processing.
    #[inline]
    pub fn requires_nlp(self) -> bool {
        matches!(self, ContentSegmentation::Semantic)
    }

    /// Returns the relative processing complexity (1 = simple, 5 = complex).
    #[inline]
    pub fn processing_complexity(self) -> u8 {
        match self {
            ContentSegmentation::None => 1,
            ContentSegmentation::Fixed => 2,
            ContentSegmentation::Semantic => 4,
        }
    }

    /// Returns the estimated processing time factor.
    #[inline]
    pub fn processing_time_factor(self) -> f32 {
        match self {
            ContentSegmentation::None => 1.0,
            ContentSegmentation::Fixed => 1.2,
            ContentSegmentation::Semantic => 2.5,
        }
    }

    /// Returns a description of the segmentation strategy.
    pub fn description(self) -> &'static str {
        match self {
            ContentSegmentation::None => "No segmentation - process entire document",
            ContentSegmentation::Semantic => "Semantic segmentation - split by meaning and context",
            ContentSegmentation::Fixed => "Fixed-size segmentation - split by character count",
        }
    }

    /// Returns whether this strategy is recommended for large documents.
    #[inline]
    pub fn recommended_for_large_documents(self) -> bool {
        matches!(
            self,
            ContentSegmentation::Semantic | ContentSegmentation::Fixed
        )
    }

    /// Returns whether this strategy preserves context between segments.
    #[inline]
    pub fn preserves_context(self) -> bool {
        matches!(self, ContentSegmentation::Semantic)
    }
}

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
    /// Returns whether segmentation is disabled.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, ContentSegmentation::None)
    }

    /// Returns whether this strategy uses semantic analysis.
    #[inline]
    pub fn is_semantic(self) -> bool {
        matches!(self, ContentSegmentation::Semantic)
    }

    /// Returns whether this strategy uses fixed-size chunks.
    #[inline]
    pub fn is_fixed(self) -> bool {
        matches!(self, ContentSegmentation::Fixed)
    }

    /// Returns whether this strategy preserves context between segments.
    #[inline]
    pub fn preserves_context(self) -> bool {
        self.is_semantic()
    }
}

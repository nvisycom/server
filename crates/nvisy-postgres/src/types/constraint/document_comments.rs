//! Document comments table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document comments table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentCommentConstraints {
    // Comment content validation constraints
    #[strum(serialize = "document_comments_content_length")]
    ContentLength,

    // Comment target validation constraints
    #[strum(serialize = "document_comments_one_target")]
    OneTarget,

    // Comment metadata constraints
    #[strum(serialize = "document_comments_metadata_size")]
    MetadataSize,

    // Comment chronological constraints
    #[strum(serialize = "document_comments_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "document_comments_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "document_comments_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl DocumentCommentConstraints {
    /// Creates a new [`DocumentCommentConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentCommentConstraints::ContentLength
            | DocumentCommentConstraints::OneTarget
            | DocumentCommentConstraints::MetadataSize => ConstraintCategory::Validation,

            DocumentCommentConstraints::UpdatedAfterCreated
            | DocumentCommentConstraints::DeletedAfterCreated
            | DocumentCommentConstraints::DeletedAfterUpdated => ConstraintCategory::Chronological,
        }
    }
}

impl From<DocumentCommentConstraints> for String {
    #[inline]
    fn from(val: DocumentCommentConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentCommentConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

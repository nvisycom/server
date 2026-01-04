//! Main document model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::documents;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, Tags};

/// Main document model representing a document within a workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Document {
    /// Unique document identifier.
    pub id: Uuid,
    /// Reference to the workspace this document belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that owns this document.
    pub account_id: Uuid,
    /// Human-readable document name.
    pub display_name: String,
    /// Detailed description of the document.
    pub description: Option<String>,
    /// Tags for document classification and search.
    pub tags: Vec<Option<String>>,
    /// Additional document metadata.
    pub metadata: serde_json::Value,
    /// Timestamp when the document was created.
    pub created_at: Timestamp,
    /// Timestamp when the document was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the document was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new document.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocument {
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Document name.
    pub display_name: Option<String>,
    /// Document description.
    pub description: Option<String>,
    /// Document tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a document.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocument {
    /// Document name.
    pub display_name: Option<String>,
    /// Document description.
    pub description: Option<Option<String>>,
    /// Document tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

impl Document {
    /// Returns the flattened tags (removing None values).
    pub fn tags(&self) -> Vec<String> {
        let tags = self.tags.clone();
        tags.into_iter().flatten().collect()
    }

    /// Returns whether the document is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the document has tags.
    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }

    /// Returns whether the document contains a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags
            .iter()
            .any(|t| t.as_ref() == Some(&tag.to_string()))
    }

    /// Returns the flattened tags (removing None values).
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.iter().filter_map(|tag| tag.clone()).collect()
    }

    /// Returns the tags as a Tags helper.
    pub fn tags_helper(&self) -> Tags {
        Tags::from_optional_strings(self.tags.clone())
    }

    /// Returns whether the document has a description.
    pub fn has_description(&self) -> bool {
        self.description
            .as_deref()
            .is_some_and(|desc| !desc.is_empty())
    }

    /// Returns whether the document has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the document's display name or a default.
    pub fn display_name_or_default(&self) -> &str {
        if self.display_name.is_empty() {
            "Untitled Document"
        } else {
            &self.display_name
        }
    }
}

impl HasCreatedAt for Document {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for Document {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for Document {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

//! Main document model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::documents;
use crate::types::{DocumentStatus, HasCreatedAt, HasDeletedAt, HasUpdatedAt, Tags};

/// Main document model representing a document within a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Document {
    /// Unique document identifier.
    pub id: Uuid,
    /// Reference to the project this document belongs to.
    pub project_id: Uuid,
    /// Reference to the account that owns this document.
    pub account_id: Uuid,
    /// Human-readable document name.
    pub display_name: String,
    /// Detailed description of the document.
    pub description: Option<String>,
    /// Tags for document classification and search.
    pub tags: Vec<Option<String>>,
    /// Current status of the document.
    pub status: DocumentStatus,
    /// Additional document metadata.
    pub metadata: serde_json::Value,
    /// Document settings.
    pub settings: serde_json::Value,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the document was soft-deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new document.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocument {
    /// Project ID.
    pub project_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Document name.
    pub display_name: Option<String>,
    /// Document description.
    pub description: Option<String>,
    /// Document tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Document status.
    pub status: Option<DocumentStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
}

/// Data for updating a document.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocument {
    /// Document name.
    pub display_name: Option<String>,
    /// Document description.
    pub description: Option<String>,
    /// Document tags.
    pub tags: Option<Vec<Option<String>>>,
    /// Document status.
    pub status: Option<DocumentStatus>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Settings.
    pub settings: Option<serde_json::Value>,
}

impl Document {
    /// Returns the flattened tags (removing None values).
    pub fn tags(&self) -> Vec<String> {
        let tags = self.tags.clone();
        tags.into_iter().flatten().collect()
    }

    /// Returns whether the document can be edited.
    pub fn is_editable(&self) -> bool {
        self.status.is_editable()
    }

    /// Returns whether the document is read-only.
    pub fn is_read_only(&self) -> bool {
        self.status.is_read_only()
    }

    /// Returns whether the document is currently being processed.
    pub fn is_processing(&self) -> bool {
        self.status.is_processing()
    }

    /// Returns whether the document is available for normal use.
    pub fn is_available(&self) -> bool {
        self.status.is_available()
    }

    /// Returns whether the document is in a completed state.
    pub fn is_completed(&self) -> bool {
        self.status.is_completed()
    }

    /// Returns whether the document is in a draft state.
    pub fn is_draft(&self) -> bool {
        self.status.is_draft()
    }

    /// Returns whether the document is archived.
    pub fn is_archived(&self) -> bool {
        self.status.is_archived()
    }

    /// Returns whether the document has encountered an error.
    pub fn has_error(&self) -> bool {
        self.status.has_error()
    }

    /// Returns whether the document is locked.
    pub fn is_locked(&self) -> bool {
        self.status.is_locked()
    }

    /// Returns whether the document is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the document can be processed.
    pub fn can_be_processed(&self) -> bool {
        self.status.can_be_processed()
    }

    /// Returns whether the document can be unlocked.
    pub fn can_be_unlocked(&self) -> bool {
        self.status.can_be_unlocked()
    }

    /// Returns whether the document can be archived.
    pub fn can_be_archived(&self) -> bool {
        self.status.can_be_archived()
    }

    /// Returns whether the document can be restored from archive.
    pub fn can_be_restored(&self) -> bool {
        self.status.can_be_restored()
    }

    /// Returns whether files can be added to this document.
    pub fn allows_file_uploads(&self) -> bool {
        self.status.allows_file_uploads()
    }

    /// Returns whether the document status indicates a stable state.
    pub fn is_stable(&self) -> bool {
        self.status.is_stable()
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

    /// Returns whether the document has custom settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the document is in a final state (cannot be modified).
    pub fn is_final(&self) -> bool {
        self.status.is_completed() || self.status.is_archived()
    }

    /// Returns whether the document allows comments.
    pub fn allows_comments(&self) -> bool {
        !self.is_deleted() && !self.status.is_locked()
    }

    /// Returns whether the document can be shared.
    pub fn can_be_shared(&self) -> bool {
        self.is_available() && !self.is_deleted()
    }

    /// Returns whether the document needs attention (has error or is processing too long).
    pub fn needs_attention(&self) -> bool {
        self.has_error() || (self.is_processing() && !self.is_recently_updated())
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
    fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

impl HasUpdatedAt for Document {
    fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

impl HasDeletedAt for Document {
    fn deleted_at(&self) -> Option<OffsetDateTime> {
        self.deleted_at
    }
}

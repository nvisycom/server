//! Main document model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::documents;
use crate::types::DocumentStatus;

/// Main document model representing a document within a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Document {
    /// Unique document identifier
    pub id: Uuid,
    /// Reference to the project this document belongs to
    pub project_id: Uuid,
    /// Reference to the account that owns this document
    pub account_id: Uuid,
    /// Human-readable document name
    pub display_name: String,
    /// Detailed description of the document
    pub description: String,
    /// Tags for document classification and search
    pub tags: Vec<Option<String>>,
    /// Current status of the document
    pub status: DocumentStatus,
    /// Whether this document is a template for creating new documents
    pub is_template: bool,
    /// Additional document metadata
    pub metadata: serde_json::Value,
    /// Document settings
    pub settings: serde_json::Value,
    /// Timestamp when the document was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the document was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new document.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocument {
    /// Project ID
    pub project_id: Uuid,
    /// Account ID
    pub account_id: Uuid,
    /// Document name
    pub display_name: Option<String>,
    /// Document description
    pub description: Option<String>,
    /// Document tags
    pub tags: Option<Vec<String>>,
    /// Document status
    pub status: Option<DocumentStatus>,
    /// Is template
    pub is_template: Option<bool>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Settings
    pub settings: Option<serde_json::Value>,
}

/// Data for updating a document.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocument {
    /// Document name
    pub display_name: Option<String>,
    /// Document description
    pub description: Option<String>,
    /// Document tags
    pub tags: Option<Vec<String>>,
    /// Document status
    pub status: Option<DocumentStatus>,
    /// Is template
    pub is_template: Option<bool>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Settings
    pub settings: Option<serde_json::Value>,
}

impl Default for NewDocument {
    fn default() -> Self {
        Self {
            project_id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            display_name: None,
            description: None,
            tags: None,
            status: Some(DocumentStatus::Draft),
            is_template: Some(false),
            metadata: Some(serde_json::Value::Object(serde_json::Map::new())),
            settings: Some(serde_json::Value::Object(serde_json::Map::new())),
        }
    }
}

impl Document {
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

    /// Returns whether this document serves as a template.
    pub fn is_template(&self) -> bool {
        self.is_template
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

    /// Returns whether the document was created recently (within last 24 hours).
    pub fn is_recently_created(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_days() < 1
    }

    /// Returns whether the document was updated recently (within last hour).
    pub fn is_recently_updated(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.updated_at;
        duration.whole_hours() < 1
    }

    /// Returns the flattened tags (removing None values).
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.iter().filter_map(|tag| tag.clone()).collect()
    }
}

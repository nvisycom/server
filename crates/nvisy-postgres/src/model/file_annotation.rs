//! File annotation model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::file_annotations;
use crate::types::{AnnotationType, HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// File annotation model representing user annotations on file content.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = file_annotations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FileAnnotation {
    /// Unique annotation identifier.
    pub id: Uuid,
    /// Reference to the file this annotation belongs to.
    pub file_id: Uuid,
    /// Reference to the account that created this annotation.
    pub account_id: Uuid,
    /// Annotation text content.
    pub content: String,
    /// Type of annotation (annotation, highlight).
    pub annotation_type: AnnotationType,
    /// Extended metadata including position/location.
    pub metadata: serde_json::Value,
    /// Timestamp when the annotation was created.
    pub created_at: Timestamp,
    /// Timestamp when the annotation was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the annotation was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new file annotation.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = file_annotations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewFileAnnotation {
    /// File ID.
    pub file_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Annotation content.
    pub content: String,
    /// Annotation type.
    pub annotation_type: Option<AnnotationType>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a file annotation.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = file_annotations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateFileAnnotation {
    /// Annotation content.
    pub content: Option<String>,
    /// Annotation type.
    pub annotation_type: Option<AnnotationType>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl FileAnnotation {
    /// Returns whether the annotation was created recently.
    pub fn is_recent(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(24))
    }

    /// Returns whether the annotation is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the annotation has been modified since creation.
    pub fn is_modified(&self) -> bool {
        self.updated_at > self.created_at
    }

    /// Returns whether the annotation has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether this is a text annotation.
    pub fn is_annotation(&self) -> bool {
        self.annotation_type.is_annotation()
    }

    /// Returns whether this is a highlight annotation.
    pub fn is_highlight(&self) -> bool {
        self.annotation_type.is_highlight()
    }

    /// Returns the content length.
    pub fn content_length(&self) -> usize {
        self.content.len()
    }

    /// Returns whether the content is empty.
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }
}

impl HasCreatedAt for FileAnnotation {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for FileAnnotation {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for FileAnnotation {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

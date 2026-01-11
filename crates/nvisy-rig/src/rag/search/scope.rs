//! Search scope for restricting vector queries.

use uuid::Uuid;

/// Search scope for vector queries.
///
/// Restricts search to specific files or documents to prevent cross-user data access.
#[derive(Debug, Clone)]
pub enum SearchScope {
    /// Search within specific files.
    Files(Vec<Uuid>),
    /// Search within specific documents (all files in those documents).
    Documents(Vec<Uuid>),
}

impl SearchScope {
    /// Creates a scope for a single file.
    pub fn file(file_id: Uuid) -> Self {
        Self::Files(vec![file_id])
    }

    /// Creates a scope for multiple files.
    pub fn files(file_ids: Vec<Uuid>) -> Self {
        Self::Files(file_ids)
    }

    /// Creates a scope for a single document.
    pub fn document(document_id: Uuid) -> Self {
        Self::Documents(vec![document_id])
    }

    /// Creates a scope for multiple documents.
    pub fn documents(document_ids: Vec<Uuid>) -> Self {
        Self::Documents(document_ids)
    }
}

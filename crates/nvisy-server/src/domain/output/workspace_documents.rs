//! Document service outputs.

use uuid::Uuid;

/// The documents actually deleted by a bulk delete, and the requested ids that
/// were not (unknown, already deleted, another workspace's, or in-progress).
pub struct BulkDeleteOutcome {
    /// Ids of the documents this request deleted.
    pub deleted: Vec<Uuid>,
    /// Requested ids that resolved to no live document to delete.
    pub skipped: Vec<Uuid>,
}

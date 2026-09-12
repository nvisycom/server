//! Blob model for PostgreSQL database operations.
//!
//! A blob is a content-addressed lump of stored bytes. It is shared: two
//! documents, audits, or intermediates with identical bytes point at the same
//! blob, and `ref_count` tracks how many references it has. Retention lives here
//! too — the reaper purges a blob's backing object once `ref_count` reaches zero
//! and its `expires_at` has passed.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_blobs;

/// A content-addressed, shared, ref-counted lump of stored bytes.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_blobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Blob {
    /// Unique blob identifier.
    pub id: Uuid,
    /// Workspace the blob's bytes belong to (dedup is scoped per workspace).
    pub workspace_id: Uuid,
    /// SHA-256 of the bytes (32 bytes); the content address.
    pub content_hash: Vec<u8>,
    /// Size of the bytes in the store.
    pub file_size_bytes: i64,
    /// Object-store path the bytes live at.
    pub storage_path: String,
    /// Object-store bucket the bytes live in.
    pub storage_bucket: String,
    /// Number of live references to this blob. The reaper reclaims the object
    /// only when this reaches zero and `expires_at` has passed.
    pub ref_count: i32,
    /// When the blob was created.
    pub created_at: Timestamp,
    /// Data-retention expiry (`None` = keep indefinitely).
    pub expires_at: Option<Timestamp>,
    /// When the backing object was reclaimed from the store. `None` means the
    /// object is still present (either live, or awaiting the reaper).
    pub purged_at: Option<Timestamp>,
}

/// Data for creating a new blob.
///
/// `ref_count` is not set here: a blob is inserted at zero and incremented by the
/// same transaction that records the first reference, so a blob never briefly
/// looks live before its referrer exists.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_blobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewBlob {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// SHA-256 of the bytes (32 bytes).
    pub content_hash: Vec<u8>,
    /// Size of the bytes.
    pub file_size_bytes: i64,
    /// Object-store path.
    pub storage_path: String,
    /// Object-store bucket.
    pub storage_bucket: String,
    /// Data-retention expiry (`None` = keep indefinitely).
    pub expires_at: Option<Timestamp>,
}

impl NewBlob {
    /// A minimal blob for `workspace_id`, for tests.
    ///
    /// Supplies the required storage fields and a valid 32-byte hash unique to
    /// this call, so each test blob is distinct and does not dedup onto another.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid) -> Self {
        let id = Uuid::now_v7();
        let mut content_hash = vec![0u8; 32];
        content_hash[..16].copy_from_slice(id.as_bytes());
        Self {
            workspace_id,
            content_hash,
            file_size_bytes: 1024,
            storage_path: format!("test/{}", id.simple()),
            storage_bucket: "test-bucket".to_owned(),
            ..Default::default()
        }
    }
}

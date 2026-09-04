//! Logical buckets and the typed object keys that address them.

mod bucket;
mod object_key;

pub use bucket::Bucket;
pub use object_key::{
    AccountAvatarKey, ArtifactKey, AuditKey, FileKey, ObjectKey, WorkspaceAvatarKey,
};

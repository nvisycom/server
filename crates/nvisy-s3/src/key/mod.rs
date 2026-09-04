//! Logical buckets and the typed object keys that address them.

mod avatar_key;
mod bucket;
mod document_key;
mod object_key;

pub use avatar_key::{AccountAvatarKey, WorkspaceAvatarKey};
pub use bucket::Bucket;
pub use document_key::{ArtifactKey, AuditKey, FileKey};
pub use object_key::ObjectKey;

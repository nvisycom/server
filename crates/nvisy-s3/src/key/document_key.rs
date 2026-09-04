//! Two-UUID keys for documents, audits, and pipeline artifacts.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use super::bucket::Bucket;
use super::object_key::{ObjectKey, decode_ids, encode_ids, strip_prefix};
use crate::error::{Error, Result};

/// A validated key for file objects.
///
/// The key is encoded as a `file_` prefix followed by URL-safe base64 of the
/// concatenated workspace ID and object ID. This produces a key like
/// `file_ABC123...` from two UUIDs (32 bytes → base64).
///
/// The `object_id` is a UUID v7 generated at upload time, providing:
/// - Time-ordered keys for efficient storage and retrieval
/// - Guaranteed uniqueness within the workspace
/// - No collision with database-generated IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for FileKey {
    const BUCKET: Bucket = Bucket::Files;
    const PREFIX: &'static str = "file_";
}

impl FileKey {
    /// Generates a new file key with a fresh UUID v7 object ID.
    ///
    /// Uses UUID v7, which is time-ordered and contains randomness, making keys
    /// both sortable and collision-resistant.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates a file key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }

    /// Regenerates the object ID with a fresh UUID v7.
    ///
    /// Useful when creating a new version of a file while keeping the same
    /// workspace association.
    pub fn regenerate(&mut self) {
        self.object_id = Uuid::now_v7();
    }

    /// Encodes the key payload as URL-safe base64.
    fn encode_payload(&self) -> String {
        encode_ids(self.workspace_id, self.object_id)
    }

    /// Decodes a key payload from URL-safe base64.
    fn decode_payload(s: &str) -> Result<Self> {
        let (workspace_id, object_id) = decode_ids(s)?;
        Ok(Self::from_parts(workspace_id, object_id))
    }
}

impl fmt::Display for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.encode_payload())
    }
}

impl FromStr for FileKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::decode_payload(strip_prefix::<Self>(s)?)
    }
}

/// A validated key for a redaction audit object.
///
/// Addresses a detection's analyzed document — the engine's detection result
/// (the audit of what was found and redacted). Encoded as an `audit_` prefix
/// followed by URL-safe base64 of the concatenated workspace ID and object ID
/// (32 bytes → base64), like [`FileKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for AuditKey {
    const BUCKET: Bucket = Bucket::Audits;
    const PREFIX: &'static str = "audit_";
}

impl AuditKey {
    /// Generates a new audit key with a fresh UUID v7 object ID.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates an audit key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }
}

impl fmt::Display for AuditKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            Self::PREFIX,
            encode_ids(self.workspace_id, self.object_id)
        )
    }
}

impl FromStr for AuditKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (workspace_id, object_id) = decode_ids(strip_prefix::<Self>(s)?)?;
        Ok(Self::from_parts(workspace_id, object_id))
    }
}

/// A validated key for a transient pipeline-artifact object.
///
/// Addresses a detection's enrichment intermediates (an image's OCR layout, an
/// audio transcript). Encoded as an `artifact_` prefix followed by URL-safe
/// base64 of the concatenated workspace ID and object ID (32 bytes → base64),
/// like [`FileKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for ArtifactKey {
    const BUCKET: Bucket = Bucket::Artifacts;
    const PREFIX: &'static str = "artifact_";
}

impl ArtifactKey {
    /// Generates a new artifact key with a fresh UUID v7 object ID.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates an artifact key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }
}

impl fmt::Display for ArtifactKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            Self::PREFIX,
            encode_ids(self.workspace_id, self.object_id)
        )
    }
}

impl FromStr for ArtifactKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (workspace_id, object_id) = decode_ids(strip_prefix::<Self>(s)?)?;
        Ok(Self::from_parts(workspace_id, object_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_key_prefix_is_file() {
        assert_eq!(FileKey::PREFIX, "file_");
    }

    #[test]
    fn file_key_generate_uses_uuid_v7() {
        let workspace_id = Uuid::new_v4();
        let key = FileKey::generate(workspace_id);
        assert_eq!(key.workspace_id, workspace_id);
        assert_eq!(key.object_id.get_version_num(), 7);
    }

    #[test]
    fn file_key_display_has_prefix_and_expected_length() {
        let key = FileKey::generate(Uuid::new_v4());
        let encoded = key.to_string();
        assert!(encoded.starts_with("file_"));
        // prefix (5) + base64 of 32 bytes (43) = 48.
        assert_eq!(encoded.len(), 48);
    }

    #[test]
    fn file_key_round_trips_through_string() {
        let key = FileKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: FileKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn file_key_rejects_wrong_prefix() {
        assert!(FileKey::from_str("audit_abc").is_err());
        assert!(FileKey::from_str("abc").is_err());
    }

    #[test]
    fn audit_key_round_trips_and_rejects_wrong_prefix() {
        let key = AuditKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: AuditKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
        assert!(key.to_string().starts_with("audit_"));
        assert!(AuditKey::from_str("file_abc").is_err());
    }

    #[test]
    fn artifact_key_round_trips_and_rejects_wrong_prefix() {
        let key = ArtifactKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: ArtifactKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
        assert!(key.to_string().starts_with("artifact_"));
        assert!(ArtifactKey::from_str("audit_abc").is_err());
    }
}

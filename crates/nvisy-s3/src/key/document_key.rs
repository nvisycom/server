//! Two-UUID keys for documents, audits, and pipeline intermediates.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use super::bucket::Bucket;
use super::object_key::{ObjectKey, decode_ids, encode_ids, strip_prefix};
use crate::error::{Error, Result};

/// A validated key for document objects.
///
/// The key is encoded as a `document_` prefix followed by URL-safe base64 of the
/// concatenated workspace ID and object ID. This produces a key like
/// `document_ABC123...` from two UUIDs (32 bytes → base64).
///
/// The `object_id` is a UUID v7 generated at upload time, providing:
/// - Time-ordered keys for efficient storage and retrieval
/// - Guaranteed uniqueness within the workspace
/// - No collision with database-generated IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for DocumentKey {
    const BUCKET: Bucket = Bucket::Documents;
    const PREFIX: &'static str = "document_";
}

impl DocumentKey {
    /// Generates a new document key with a fresh UUID v7 object ID.
    ///
    /// Uses UUID v7, which is time-ordered and contains randomness, making keys
    /// both sortable and collision-resistant.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates a document key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }

    /// Regenerates the object ID with a fresh UUID v7.
    ///
    /// Useful when creating a new version of a document while keeping the same
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

impl fmt::Display for DocumentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.encode_payload())
    }
}

impl FromStr for DocumentKey {
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
/// (32 bytes → base64), like [`DocumentKey`].
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

/// A validated key for a transient pipeline-intermediate object.
///
/// Addresses a detection's enrichment intermediates (an image's OCR layout, an
/// audio transcript). Encoded as an `intermediate_` prefix followed by URL-safe
/// base64 of the concatenated workspace ID and object ID (32 bytes → base64),
/// like [`DocumentKey`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntermediateKey {
    pub workspace_id: Uuid,
    pub object_id: Uuid,
}

impl ObjectKey for IntermediateKey {
    const BUCKET: Bucket = Bucket::Intermediates;
    const PREFIX: &'static str = "intermediate_";
}

impl IntermediateKey {
    /// Generates a new intermediate key with a fresh UUID v7 object ID.
    pub fn generate(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id: Uuid::now_v7(),
        }
    }

    /// Creates an intermediate key from existing IDs (for parsing stored keys).
    pub fn from_parts(workspace_id: Uuid, object_id: Uuid) -> Self {
        Self {
            workspace_id,
            object_id,
        }
    }
}

impl fmt::Display for IntermediateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            Self::PREFIX,
            encode_ids(self.workspace_id, self.object_id)
        )
    }
}

impl FromStr for IntermediateKey {
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
    fn document_key_prefix_is_document() {
        assert_eq!(DocumentKey::PREFIX, "document_");
    }

    #[test]
    fn document_key_generate_uses_uuid_v7() {
        let workspace_id = Uuid::new_v4();
        let key = DocumentKey::generate(workspace_id);
        assert_eq!(key.workspace_id, workspace_id);
        assert_eq!(key.object_id.get_version_num(), 7);
    }

    #[test]
    fn document_key_display_has_prefix_and_expected_length() {
        let key = DocumentKey::generate(Uuid::new_v4());
        let encoded = key.to_string();
        assert!(encoded.starts_with("document_"));
        // prefix (9) + base64 of 32 bytes (43) = 52.
        assert_eq!(encoded.len(), 52);
    }

    #[test]
    fn document_key_round_trips_through_string() {
        let key = DocumentKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: DocumentKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn document_key_rejects_wrong_prefix() {
        assert!(DocumentKey::from_str("audit_abc").is_err());
        assert!(DocumentKey::from_str("abc").is_err());
    }

    #[test]
    fn audit_key_round_trips_and_rejects_wrong_prefix() {
        let key = AuditKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: AuditKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
        assert!(key.to_string().starts_with("audit_"));
        assert!(AuditKey::from_str("document_abc").is_err());
    }

    #[test]
    fn intermediate_key_round_trips_and_rejects_wrong_prefix() {
        let key = IntermediateKey::from_parts(Uuid::new_v4(), Uuid::new_v4());
        let decoded: IntermediateKey = key.to_string().parse().unwrap();
        assert_eq!(key, decoded);
        assert!(key.to_string().starts_with("intermediate_"));
        assert!(IntermediateKey::from_str("audit_abc").is_err());
    }
}

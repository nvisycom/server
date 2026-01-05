//! Document key for NATS object storage.

use std::fmt;
use std::str::FromStr;

use base64::prelude::*;
use uuid::Uuid;

use crate::{Error, Result};

/// A validated key for document objects in NATS object storage.
///
/// The key is encoded as URL-safe base64 of the concatenated workspace ID and object ID.
/// This produces a compact 43-character key from two UUIDs (32 bytes → base64).
///
/// The `object_id` is a UUID v7 generated at upload time, providing:
/// - Time-ordered keys for efficient storage and retrieval
/// - Guaranteed uniqueness within the workspace
/// - No collision with database-generated IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentKey {
    workspace_id: Uuid,
    object_id: Uuid,
}

impl DocumentKey {
    /// Generates a new document key with a fresh UUID v7 object ID.
    ///
    /// Uses UUID v7 which is time-ordered and contains randomness,
    /// making keys both sortable and collision-resistant.
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

    /// Returns the workspace ID.
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Returns the object ID (the UUID used for NATS storage).
    pub fn object_id(&self) -> Uuid {
        self.object_id
    }

    /// Encodes the key as URL-safe base64.
    fn encode(&self) -> String {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(self.workspace_id.as_bytes());
        bytes[16..].copy_from_slice(self.object_id.as_bytes());
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Decodes a key from URL-safe base64.
    fn decode(s: &str) -> Result<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(s).map_err(|e| {
            Error::operation("parse_key", format!("Invalid base64 encoding: {}", e))
        })?;

        if bytes.len() != 32 {
            return Err(Error::operation(
                "parse_key",
                format!("Invalid key length: expected 32 bytes, got {}", bytes.len()),
            ));
        }

        let workspace_id = Uuid::from_slice(&bytes[..16])
            .map_err(|e| Error::operation("parse_key", format!("Invalid workspace UUID: {}", e)))?;

        let object_id = Uuid::from_slice(&bytes[16..])
            .map_err(|e| Error::operation("parse_key", format!("Invalid object UUID: {}", e)))?;

        Ok(Self::from_parts(workspace_id, object_id))
    }
}

impl fmt::Display for DocumentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl FromStr for DocumentKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::decode(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_key_generate() {
        let workspace_id = Uuid::new_v4();

        let key = DocumentKey::generate(workspace_id);

        assert_eq!(key.workspace_id(), workspace_id);
        // object_id should be a valid UUID v7 (starts with version nibble 7)
        assert_eq!(key.object_id().get_version_num(), 7);
    }

    #[test]
    fn test_document_key_from_parts() {
        let workspace_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();

        let key = DocumentKey::from_parts(workspace_id, object_id);

        assert_eq!(key.workspace_id(), workspace_id);
        assert_eq!(key.object_id(), object_id);
    }

    #[test]
    fn test_document_key_display_is_base64() {
        let workspace_id = Uuid::new_v4();

        let key = DocumentKey::generate(workspace_id);
        let encoded = key.to_string();

        // URL-safe base64 without padding: 32 bytes → 43 chars
        assert_eq!(encoded.len(), 43);
        // Should only contain URL-safe base64 characters
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn test_document_key_roundtrip() {
        let workspace_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();

        let key = DocumentKey::from_parts(workspace_id, object_id);
        let encoded = key.to_string();
        let decoded: DocumentKey = encoded.parse().unwrap();

        assert_eq!(decoded.workspace_id(), workspace_id);
        assert_eq!(decoded.object_id(), object_id);
        assert_eq!(key, decoded);
    }

    #[test]
    fn test_document_key_uniqueness() {
        let workspace_id = Uuid::new_v4();

        // Generate multiple keys for the same workspace
        let key1 = DocumentKey::generate(workspace_id);
        let key2 = DocumentKey::generate(workspace_id);

        // Each should have a unique object_id
        assert_ne!(key1.object_id(), key2.object_id());
        assert_ne!(key1.to_string(), key2.to_string());
    }

    #[test]
    fn test_document_key_from_str_invalid() {
        // Invalid base64
        assert!(DocumentKey::from_str("not-valid-base64!!!").is_err());

        // Too short
        assert!(DocumentKey::from_str("abc").is_err());

        // Valid base64 but wrong length
        assert!(DocumentKey::from_str("YWJjZGVm").is_err());
    }
}

//! Two-UUID keys for documents, audits, and pipeline intermediates.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use super::bucket::Bucket;
use super::object_key::{ObjectKey, decode_ids, encode_ids, strip_prefix};
use crate::error::{Error, Result};

/// Defines a workspace-scoped key: a prefix followed by URL-safe base64 of the
/// concatenated workspace id and object id (32 bytes → 43 characters).
macro_rules! workspace_object_key {
    ($(#[$meta:meta])* $name:ident, $prefix:literal, $bucket:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            /// The workspace the object belongs to.
            pub workspace_id: Uuid,
            /// The object's own id, a UUID v7.
            pub object_id: Uuid,
        }

        impl ObjectKey for $name {
            const BUCKET: Bucket = $bucket;
            const PREFIX: &'static str = $prefix;
        }

        impl $name {
            /// A new key in `workspace_id` with a fresh UUID v7 object id, so keys
            /// are time-ordered and collision-resistant.
            #[must_use]
            pub fn generate(workspace_id: Uuid) -> Self {
                Self {
                    workspace_id,
                    object_id: Uuid::now_v7(),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", Self::PREFIX, encode_ids(self.workspace_id, self.object_id))
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self> {
                let (workspace_id, object_id) = decode_ids(strip_prefix::<Self>(s)?)?;
                Ok(Self {
                    workspace_id,
                    object_id,
                })
            }
        }
    };
}

workspace_object_key!(
    /// A key for an uploaded or redacted document, `document_{ids}`.
    DocumentKey,
    "document_",
    Bucket::Documents
);

workspace_object_key!(
    /// A key for a detection's audit: the engine's record of what was found and
    /// redacted, `audit_{ids}`.
    AuditKey,
    "audit_",
    Bucket::Audits
);

workspace_object_key!(
    /// A key for a detection's transient enrichment intermediates (an image's
    /// OCR layout, an audio transcript), `intermediate_{ids}`.
    IntermediateKey,
    "intermediate_",
    Bucket::Intermediates
);

#[cfg(test)]
mod tests {
    use super::*;

    fn key<K: ObjectKey>(build: impl Fn(Uuid, Uuid) -> K) -> K {
        build(Uuid::new_v4(), Uuid::now_v7())
    }

    #[test]
    fn generate_uses_uuid_v7() {
        let workspace_id = Uuid::new_v4();
        let key = DocumentKey::generate(workspace_id);
        assert_eq!(key.workspace_id, workspace_id);
        assert_eq!(key.object_id.get_version_num(), 7);
    }

    #[test]
    fn display_is_prefix_then_43_base64_characters() {
        let encoded = DocumentKey::generate(Uuid::new_v4()).to_string();
        assert!(encoded.starts_with("document_"));
        assert_eq!(encoded.len(), "document_".len() + 43);
    }

    #[test]
    fn every_key_round_trips_and_rejects_another_prefix() {
        let document = key(|workspace_id, object_id| DocumentKey {
            workspace_id,
            object_id,
        });
        assert_eq!(
            document.to_string().parse::<DocumentKey>().unwrap(),
            document
        );
        assert!(DocumentKey::from_str("audit_abc").is_err());
        assert!(DocumentKey::from_str("abc").is_err());

        let audit = key(|workspace_id, object_id| AuditKey {
            workspace_id,
            object_id,
        });
        assert_eq!(audit.to_string().parse::<AuditKey>().unwrap(), audit);
        assert!(AuditKey::from_str("document_abc").is_err());

        let intermediate = key(|workspace_id, object_id| IntermediateKey {
            workspace_id,
            object_id,
        });
        assert_eq!(
            intermediate.to_string().parse::<IntermediateKey>().unwrap(),
            intermediate
        );
        assert!(IntermediateKey::from_str("audit_abc").is_err());
    }
}

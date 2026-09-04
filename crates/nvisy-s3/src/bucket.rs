//! Logical stores within the single S3 bucket.

/// A logical store — a class of first-party object with its own key prefix and
/// key type inside the shared S3 bucket.
///
/// Each variant maps to a stable [`name`](Self::name) (recorded in the
/// `workspace_files` `storage_bucket` column, so a stored object can be routed
/// back to its store) and an S3 key [`prefix`](Self::prefix) that namespaces its
/// objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Uploaded and processed document files.
    Files,
    /// Detection analysis blobs (audits, review audits, intermediates).
    Audits,
    /// Account avatars.
    AccountAvatars,
    /// Workspace avatars (logos).
    WorkspaceAvatars,
}

impl Bucket {
    /// The stable identifier stored in `workspace_files.storage_bucket`, so a
    /// stored object can be routed back to the store that holds it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Files => "DOCUMENT_FILES",
            Self::Audits => "DOCUMENT_AUDITS",
            Self::AccountAvatars => "ACCOUNT_AVATARS",
            Self::WorkspaceAvatars => "WORKSPACE_AVATARS",
        }
    }

    /// The S3 key prefix under which this store's objects live within the shared
    /// bucket. A full key is `"{prefix}/{object-key}"`.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Audits => "audits",
            Self::AccountAvatars => "account-avatars",
            Self::WorkspaceAvatars => "workspace-avatars",
        }
    }

    /// The store identified by a stored [`name`](Self::name), or `None` if the
    /// name is unrecognized (a stale or corrupt `storage_bucket` value).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "DOCUMENT_FILES" => Some(Self::Files),
            "DOCUMENT_AUDITS" => Some(Self::Audits),
            "ACCOUNT_AVATARS" => Some(Self::AccountAvatars),
            "WORKSPACE_AVATARS" => Some(Self::WorkspaceAvatars),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant's stored name parses back to the same variant, so a
    /// persisted `storage_bucket` always routes an object to its own store.
    #[test]
    fn name_round_trips_through_from_name() {
        for bucket in [
            Bucket::Files,
            Bucket::Audits,
            Bucket::AccountAvatars,
            Bucket::WorkspaceAvatars,
        ] {
            assert_eq!(Bucket::from_name(bucket.name()), Some(bucket));
        }
    }

    #[test]
    fn from_name_rejects_unknown() {
        assert_eq!(Bucket::from_name("NOPE"), None);
        assert_eq!(Bucket::from_name(""), None);
    }
}

//! `{id}_{version}` keys for account and workspace avatars.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use super::bucket::Bucket;
use super::object_key::{ObjectKey, parse_error, split_id_version};
use crate::error::{Error, Result};

/// Defines an owner-scoped avatar key, `{prefix}{owner_id}_{version}`, where
/// `version` is drawn randomly per upload. Each version is a distinct object, so
/// a versioned URL maps to immutable bytes and a stale version simply does not
/// exist.
macro_rules! avatar_key {
    ($(#[$meta:meta])* $name:ident, $owner:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            /// The avatar's owner.
            pub $owner: Uuid,
            /// The avatar version.
            pub version: String,
        }

        impl ObjectKey for $name {
            const BUCKET: Bucket = Bucket::Avatars;
            const PREFIX: &'static str = $prefix;
        }

        impl $name {
            /// A key for one version of the owner's avatar.
            pub fn new($owner: Uuid, version: impl Into<String>) -> Self {
                Self {
                    $owner,
                    version: version.into(),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}_{}", Self::PREFIX, self.$owner, self.version)
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self> {
                let (id, version) = split_id_version::<Self>(s)?;
                let $owner =
                    Uuid::parse_str(id).map_err(|e| parse_error(format!("Invalid owner UUID: {e}")))?;
                Ok(Self::new($owner, version))
            }
        }
    };
}

avatar_key!(
    /// A key for one version of an account's avatar, `account_{account_id}_{version}`.
    AccountAvatarKey,
    account_id,
    "account_"
);

avatar_key!(
    /// A key for one version of a workspace's avatar,
    /// `workspace_{workspace_id}_{version}`.
    WorkspaceAvatarKey,
    workspace_id,
    "workspace_"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_avatar_round_trips() {
        let account_id = Uuid::new_v4();
        let key = AccountAvatarKey::new(account_id, "abc123");
        assert_eq!(key.to_string(), format!("account_{account_id}_abc123"));
        assert_eq!(key.to_string().parse::<AccountAvatarKey>().unwrap(), key);
    }

    #[test]
    fn workspace_avatar_round_trips() {
        let key = WorkspaceAvatarKey::new(Uuid::new_v4(), "v9");
        assert_eq!(key.to_string().parse::<WorkspaceAvatarKey>().unwrap(), key);
    }

    #[test]
    fn avatar_rejects_bad_input() {
        assert!(AccountAvatarKey::from_str("file_abc").is_err());
        assert!(AccountAvatarKey::from_str("account_not-a-uuid").is_err());
    }
}

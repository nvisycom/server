//! Avatar key for NATS object storage.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use crate::{Error, Result};

/// A validated key for avatar objects in NATS object storage.
///
/// The key format is simply the account ID as a string, since avatars
/// are uniquely identified by their owning account.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AvatarKey {
    account_id: Uuid,
}

impl AvatarKey {
    /// Creates a new avatar key for an account.
    pub fn new(account_id: Uuid) -> Self {
        Self { account_id }
    }

    /// Returns the account ID.
    pub fn account_id(&self) -> Uuid {
        self.account_id
    }
}

impl fmt::Display for AvatarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.account_id)
    }
}

impl FromStr for AvatarKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let account_id = Uuid::parse_str(s)
            .map_err(|e| Error::operation("parse_key", format!("Invalid account UUID: {}", e)))?;
        Ok(Self::new(account_id))
    }
}

impl From<Uuid> for AvatarKey {
    fn from(account_id: Uuid) -> Self {
        Self::new(account_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avatar_key_new() {
        let account_id = Uuid::new_v4();
        let key = AvatarKey::new(account_id);
        assert_eq!(key.account_id(), account_id);
    }

    #[test]
    fn test_avatar_key_display() {
        let account_id = Uuid::new_v4();
        let key = AvatarKey::new(account_id);
        assert_eq!(key.to_string(), account_id.to_string());
    }

    #[test]
    fn test_avatar_key_roundtrip() {
        let account_id = Uuid::new_v4();
        let key = AvatarKey::new(account_id);
        let encoded = key.to_string();
        let decoded: AvatarKey = encoded.parse().unwrap();
        assert_eq!(decoded.account_id(), account_id);
    }

    #[test]
    fn test_avatar_key_from_uuid() {
        let account_id = Uuid::new_v4();
        let key: AvatarKey = account_id.into();
        assert_eq!(key.account_id(), account_id);
    }

    #[test]
    fn test_avatar_key_from_str_invalid() {
        assert!(AvatarKey::from_str("not-a-uuid").is_err());
    }
}

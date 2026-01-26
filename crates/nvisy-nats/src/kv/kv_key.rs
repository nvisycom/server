//! Key-value key types and traits.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use crate::Error;

/// Marker trait for KV key types.
///
/// This trait defines how keys are formatted for storage in NATS KV.
pub trait KvKey: fmt::Debug + fmt::Display + FromStr + Clone + Send + Sync + 'static {}

/// Key for chat history sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionKey(pub Uuid);

impl KvKey for SessionKey {}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id =
            Uuid::parse_str(s).map_err(|e| Error::operation("parse_session_key", e.to_string()))?;
        Ok(Self(id))
    }
}

impl From<Uuid> for SessionKey {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

/// Key for API tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenKey(pub Uuid);

impl KvKey for TokenKey {}

impl fmt::Display for TokenKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TokenKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id =
            Uuid::parse_str(s).map_err(|e| Error::operation("parse_token_key", e.to_string()))?;
        Ok(Self(id))
    }
}

impl From<Uuid> for TokenKey {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_key_roundtrip() {
        let id = Uuid::nil();
        let key = SessionKey(id);
        let s = key.to_string();
        let parsed: SessionKey = s.parse().unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_token_key_roundtrip() {
        let id = Uuid::nil();
        let key = TokenKey(id);
        let s = key.to_string();
        let parsed: TokenKey = s.parse().unwrap();
        assert_eq!(key, parsed);
    }
}

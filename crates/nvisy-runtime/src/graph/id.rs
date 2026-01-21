//! Node identifier type.

use std::str::FromStr;

use derive_more::{Debug, Display, From, Into};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a node in a workflow graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Debug, Display, From, Into)]
#[debug("{_0}")]
#[display("{_0}")]
#[serde(transparent)]
pub struct NodeId(Uuid);

impl NodeId {
    /// Creates a new random node ID.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates a node ID from an existing UUID.
    #[inline]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    #[inline]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Returns the UUID as bytes.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for NodeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::from_str(s)?))
    }
}

impl AsRef<Uuid> for NodeId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

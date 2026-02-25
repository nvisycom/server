//! Unique content source identifier backed by UUIDv7.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque identifier for a piece of content, backed by a UUIDv7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentSource(Uuid);

impl ContentSource {
    /// Generate a new time-ordered content source id (UUIDv7).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ContentSource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ContentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

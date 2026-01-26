//! Credentials management for AI providers.
//!
//! This module provides a registry for storing and retrieving credentials
//! used by AI providers (completion, embedding) during workflow execution.

use std::collections::HashMap;

use derive_more::From;
use nvisy_rig::provider::{CompletionCredentials, EmbeddingCredentials};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;
use uuid::Uuid;

use crate::error::{Error, Result};

/// AI provider credentials.
#[derive(Debug, Clone, From, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ProviderCredentials {
    /// Completion provider credentials.
    Completion(CompletionCredentials),
    /// Embedding provider credentials.
    Embedding(EmbeddingCredentials),
}

impl ProviderCredentials {
    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Converts to completion credentials if applicable.
    pub fn into_completion_credentials(self) -> Result<CompletionCredentials> {
        match self {
            Self::Completion(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected completion credentials, got '{}'",
                other.kind()
            ))),
        }
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding_credentials(self) -> Result<EmbeddingCredentials> {
        match self {
            Self::Embedding(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected embedding credentials, got '{}'",
                other.kind()
            ))),
        }
    }
}

/// In-memory registry for AI provider credentials.
///
/// Credentials are stored by UUID and can be retrieved during workflow compilation.
#[derive(Debug, Clone, Default)]
pub struct CredentialsRegistry {
    credentials: HashMap<Uuid, ProviderCredentials>,
}

impl CredentialsRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers credentials with a UUID.
    pub fn register(&mut self, id: Uuid, creds: ProviderCredentials) {
        self.credentials.insert(id, creds);
    }

    /// Retrieves credentials by UUID.
    pub fn get(&self, id: Uuid) -> Result<&ProviderCredentials> {
        self.credentials
            .get(&id)
            .ok_or_else(|| Error::CredentialsNotFound(id))
    }

    /// Removes credentials by UUID.
    pub fn remove(&mut self, id: Uuid) -> Option<ProviderCredentials> {
        self.credentials.remove(&id)
    }

    /// Returns the number of registered credentials.
    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    /// Returns true if no credentials are registered.
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }

    /// Clears all credentials.
    pub fn clear(&mut self) {
        self.credentials.clear();
    }
}

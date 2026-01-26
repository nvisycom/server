//! Credentials management for workflow providers.
//!
//! This module provides a registry for storing and retrieving credentials
//! used by AI providers (completion, embedding) and DAL providers (postgres, s3, pinecone)
//! during workflow execution.

use std::collections::HashMap;

use derive_more::From;
use nvisy_dal::contexts::{AnyContext, ObjectContext, RelationalContext, VectorContext};
use nvisy_dal::provider::{
    AnyCredentials, PineconeCredentials, PostgresCredentials, S3Credentials,
};
use nvisy_rig::provider::{CompletionCredentials, EmbeddingCredentials};
use serde::{Deserialize, Serialize};
use strum::AsRefStr;
use uuid::Uuid;

use crate::error::{Error, Result};

/// AI provider credentials.
#[derive(Debug, Clone, From, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AiCredentials {
    /// Completion provider credentials.
    Completion(CompletionCredentials),
    /// Embedding provider credentials.
    Embedding(EmbeddingCredentials),
}

impl AiCredentials {
    /// Converts to completion credentials if applicable.
    pub fn into_completion(self) -> Result<CompletionCredentials> {
        match self {
            Self::Completion(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected completion credentials, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding(self) -> Result<EmbeddingCredentials> {
        match self {
            Self::Embedding(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected embedding credentials, got '{}'",
                other.as_ref()
            ))),
        }
    }
}

/// DAL provider credentials with context for data input/output.
#[derive(Debug, Clone, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DalCredentials {
    /// PostgreSQL credentials with relational context.
    Postgres {
        credentials: PostgresCredentials,
        context: RelationalContext,
    },
    /// S3 credentials with object context.
    S3 {
        credentials: S3Credentials,
        context: ObjectContext,
    },
    /// Pinecone credentials with vector context.
    Pinecone {
        credentials: PineconeCredentials,
        context: VectorContext,
    },
}

impl DalCredentials {
    /// Returns the credentials portion as AnyCredentials.
    pub fn credentials(&self) -> AnyCredentials {
        match self {
            Self::Postgres { credentials, .. } => AnyCredentials::Postgres(credentials.clone()),
            Self::S3 { credentials, .. } => AnyCredentials::S3(credentials.clone()),
            Self::Pinecone { credentials, .. } => AnyCredentials::Pinecone(credentials.clone()),
        }
    }

    /// Returns the context portion as AnyContext.
    pub fn context(&self) -> AnyContext {
        match self {
            Self::Postgres { context, .. } => AnyContext::Relational(context.clone()),
            Self::S3 { context, .. } => AnyContext::Object(context.clone()),
            Self::Pinecone { context, .. } => AnyContext::Vector(context.clone()),
        }
    }
}

/// All provider credentials (AI and DAL).
#[derive(Debug, Clone, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ProviderCredentials {
    /// AI provider credentials.
    Ai(AiCredentials),
    /// DAL provider credentials.
    Dal(DalCredentials),
}

impl ProviderCredentials {
    /// Converts to AI credentials if applicable.
    pub fn into_ai(self) -> Result<AiCredentials> {
        match self {
            Self::Ai(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected AI credentials, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to DAL credentials if applicable.
    pub fn into_dal(self) -> Result<DalCredentials> {
        match self {
            Self::Dal(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected DAL credentials, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to completion credentials if applicable.
    pub fn into_completion_credentials(self) -> Result<CompletionCredentials> {
        self.into_ai()?.into_completion()
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding_credentials(self) -> Result<EmbeddingCredentials> {
        self.into_ai()?.into_embedding()
    }

    /// Converts to DAL credentials and context if applicable.
    pub fn into_dal_credentials(self) -> Result<(AnyCredentials, AnyContext)> {
        let dal = self.into_dal()?;
        Ok((dal.credentials(), dal.context()))
    }
}

/// In-memory registry for provider credentials.
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
    pub fn register(&mut self, id: Uuid, creds: impl Into<ProviderCredentials>) {
        self.credentials.insert(id, creds.into());
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

// Convenience From implementations for registering credentials directly
impl From<AiCredentials> for ProviderCredentials {
    fn from(creds: AiCredentials) -> Self {
        Self::Ai(creds)
    }
}

impl From<DalCredentials> for ProviderCredentials {
    fn from(creds: DalCredentials) -> Self {
        Self::Dal(creds)
    }
}

impl From<CompletionCredentials> for ProviderCredentials {
    fn from(creds: CompletionCredentials) -> Self {
        Self::Ai(AiCredentials::Completion(creds))
    }
}

impl From<EmbeddingCredentials> for ProviderCredentials {
    fn from(creds: EmbeddingCredentials) -> Self {
        Self::Ai(AiCredentials::Embedding(creds))
    }
}

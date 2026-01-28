//! Connection management for workflow providers.
//!
//! This module provides types and utilities for managing provider connections
//! used during workflow execution. A connection bundles credentials with context
//! (configuration) for AI providers (completion, embedding) and DAL providers
//! (postgres, s3, pinecone, etc.).
//!
//! # Key Derivation
//!
//! When loading connections from PostgreSQL, each workspace has its own
//! encryption key derived from a master key using HKDF-SHA256. This ensures
//! that connection data from different workspaces is encrypted with different
//! keys, providing cryptographic isolation between workspaces.

use std::collections::HashMap;

use nvisy_core::crypto::{EncryptionKey, decrypt_json};
use nvisy_dal::contexts::{AnyContext, ObjectContext, RelationalContext, VectorContext};
use nvisy_dal::provider::{
    AnyCredentials, MilvusCredentials, PineconeCredentials, PostgresCredentials, QdrantCredentials,
    S3Credentials, WeaviateCredentials,
};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceConnectionRepository;
use nvisy_postgres::types::OffsetPagination;
use nvisy_rig::provider::Credentials as AiCredentials;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;
use uuid::Uuid;

use crate::error::{Error, Result};

/// AI provider connection.
///
/// AI connections only require credentials (API keys) without additional
/// context, since they call external APIs rather than read/write data.
/// The same credentials can be used for both completion and embedding,
/// depending on the provider's capabilities.
#[derive(Debug, Clone, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AiConnection {
    /// Completion provider (e.g., OpenAI GPT, Anthropic Claude).
    Completion(AiCredentials),
    /// Embedding provider (e.g., OpenAI Ada, Cohere).
    Embedding(AiCredentials),
}

impl AiConnection {
    /// Converts to completion credentials if applicable.
    pub fn into_completion(self) -> Result<AiCredentials> {
        match self {
            Self::Completion(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected completion connection, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding(self) -> Result<AiCredentials> {
        match self {
            Self::Embedding(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected embedding connection, got '{}'",
                other.as_ref()
            ))),
        }
    }
}

/// DAL provider connection with credentials and context for data input/output.
///
/// Unlike AI connections, DAL connections require both credentials and context
/// because they interact with external data sources (databases, object stores,
/// vector databases) that need configuration for what data to read/write.
#[derive(Debug, Clone, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DalConnection {
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
    /// Qdrant credentials with vector context.
    Qdrant {
        credentials: QdrantCredentials,
        context: VectorContext,
    },
    /// Milvus credentials with vector context.
    Milvus {
        credentials: MilvusCredentials,
        context: VectorContext,
    },
    /// Weaviate credentials with vector context.
    Weaviate {
        credentials: WeaviateCredentials,
        context: VectorContext,
    },
}

impl DalConnection {
    /// Returns the credentials portion as AnyCredentials.
    pub fn credentials(&self) -> AnyCredentials {
        match self {
            Self::Postgres { credentials, .. } => AnyCredentials::Postgres(credentials.clone()),
            Self::S3 { credentials, .. } => AnyCredentials::S3(credentials.clone()),
            Self::Pinecone { credentials, .. } => AnyCredentials::Pinecone(credentials.clone()),
            Self::Qdrant { credentials, .. } => AnyCredentials::Qdrant(credentials.clone()),
            Self::Milvus { credentials, .. } => AnyCredentials::Milvus(credentials.clone()),
            Self::Weaviate { credentials, .. } => AnyCredentials::Weaviate(credentials.clone()),
        }
    }

    /// Returns the context portion as AnyContext.
    pub fn context(&self) -> AnyContext {
        match self {
            Self::Postgres { context, .. } => AnyContext::Relational(context.clone()),
            Self::S3 { context, .. } => AnyContext::Object(context.clone()),
            Self::Pinecone { context, .. }
            | Self::Qdrant { context, .. }
            | Self::Milvus { context, .. }
            | Self::Weaviate { context, .. } => AnyContext::Vector(context.clone()),
        }
    }
}

/// All provider connections (AI and DAL).
///
/// This is the top-level type that gets encrypted and stored in the database
/// as a workspace connection.
#[derive(Debug, Clone, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ProviderConnection {
    /// AI provider connection.
    Ai(AiConnection),
    /// DAL provider connection.
    Dal(DalConnection),
}

impl ProviderConnection {
    /// Converts to AI connection if applicable.
    pub fn into_ai(self) -> Result<AiConnection> {
        match self {
            Self::Ai(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected AI connection, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to DAL connection if applicable.
    pub fn into_dal(self) -> Result<DalConnection> {
        match self {
            Self::Dal(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected DAL connection, got '{}'",
                other.as_ref()
            ))),
        }
    }

    /// Converts to completion credentials if applicable.
    pub fn into_completion_credentials(self) -> Result<AiCredentials> {
        self.into_ai()?.into_completion()
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding_credentials(self) -> Result<AiCredentials> {
        self.into_ai()?.into_embedding()
    }

    /// Converts to DAL credentials and context if applicable.
    pub fn into_dal_credentials(self) -> Result<(AnyCredentials, AnyContext)> {
        let dal = self.into_dal()?;
        Ok((dal.credentials(), dal.context()))
    }
}

impl From<AiConnection> for ProviderConnection {
    fn from(conn: AiConnection) -> Self {
        Self::Ai(conn)
    }
}

impl From<DalConnection> for ProviderConnection {
    fn from(conn: DalConnection) -> Self {
        Self::Dal(conn)
    }
}

/// In-memory registry for provider connections.
///
/// Connections are stored by UUID and can be retrieved during workflow compilation.
#[derive(Debug, Clone, Default)]
pub struct ConnectionRegistry {
    connections: HashMap<Uuid, ProviderConnection>,
}

impl ConnectionRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a connection with a UUID.
    pub fn register(&mut self, id: Uuid, conn: impl Into<ProviderConnection>) {
        self.connections.insert(id, conn.into());
    }

    /// Retrieves a connection by UUID.
    pub fn get(&self, id: Uuid) -> Result<&ProviderConnection> {
        self.connections
            .get(&id)
            .ok_or_else(|| Error::ConnectionNotFound(id))
    }

    /// Removes a connection by UUID.
    pub fn remove(&mut self, id: Uuid) -> Option<ProviderConnection> {
        self.connections.remove(&id)
    }

    /// Returns the number of registered connections.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Returns true if no connections are registered.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Clears all connections.
    pub fn clear(&mut self) {
        self.connections.clear();
    }
}

/// Loads connections from PostgreSQL workspace connections.
///
/// This loader retrieves encrypted connection data from the database,
/// decrypts it using workspace-derived encryption keys, and populates a
/// [`ConnectionRegistry`] for use during workflow execution.
///
/// # Key Derivation
///
/// The loader stores a master encryption key and derives workspace-specific
/// keys using HKDF-SHA256 with the workspace ID as salt. This provides:
///
/// - **Workspace isolation**: Each workspace's connections are encrypted with
///   a unique derived key
/// - **Single secret management**: Only one master key needs to be stored and
///   rotated
/// - **Deterministic derivation**: The same workspace always produces the same
///   derived key from a given master key
#[derive(Clone)]
pub struct PgConnectionLoader {
    client: PgClient,
    /// Master encryption key used to derive workspace-specific keys.
    master_key: EncryptionKey,
}

impl PgConnectionLoader {
    /// Creates a new PostgreSQL connection loader.
    ///
    /// # Arguments
    ///
    /// * `client` - PostgreSQL client for database access
    /// * `master_key` - Master key for deriving workspace-specific encryption keys
    pub fn new(client: PgClient, master_key: EncryptionKey) -> Self {
        Self { client, master_key }
    }

    /// Derives a workspace-specific encryption key from the master key.
    fn derive_key(&self, workspace_id: Uuid) -> EncryptionKey {
        self.master_key.derive_workspace_key(workspace_id)
    }

    /// Loads all active connections for a workspace into a registry.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The workspace to load connections for
    ///
    /// # Returns
    ///
    /// A registry populated with all active workspace connections.
    pub async fn load_workspace_connections(
        &self,
        workspace_id: Uuid,
    ) -> Result<ConnectionRegistry> {
        let mut conn = self
            .client
            .get_connection()
            .await
            .map_err(Error::from_postgres)?;

        let pagination = OffsetPagination::new(1000, 0);
        let connections = conn
            .offset_list_workspace_connections(workspace_id, pagination)
            .await
            .map_err(Error::from_postgres)?;

        // Derive workspace-specific key for decryption
        let workspace_key = self.derive_key(workspace_id);
        let mut registry = ConnectionRegistry::new();

        for connection in connections {
            let provider_conn: ProviderConnection =
                decrypt_json(&workspace_key, &connection.encrypted_data)
                    .map_err(|e| Error::Internal(format!("failed to decrypt connection: {e}")))?;

            registry.register(connection.id, provider_conn);
        }

        Ok(registry)
    }

    /// Loads a specific connection by ID.
    ///
    /// # Arguments
    ///
    /// * `connection_id` - The connection UUID to load
    ///
    /// # Returns
    ///
    /// The decrypted provider connection.
    pub async fn load_connection(&self, connection_id: Uuid) -> Result<ProviderConnection> {
        let mut conn = self
            .client
            .get_connection()
            .await
            .map_err(Error::from_postgres)?;

        let connection = conn
            .find_workspace_connection_by_id(connection_id)
            .await
            .map_err(Error::from_postgres)?
            .ok_or(Error::ConnectionNotFound(connection_id))?;

        // Derive workspace-specific key for decryption
        let workspace_key = self.derive_key(connection.workspace_id);
        let provider_conn: ProviderConnection =
            decrypt_json(&workspace_key, &connection.encrypted_data)
                .map_err(|e| Error::Internal(format!("failed to decrypt connection: {e}")))?;

        Ok(provider_conn)
    }

    /// Loads multiple connections by their IDs into a registry.
    ///
    /// This method fetches each connection individually to ensure the correct
    /// workspace-derived key is used for decryption. Connections may belong
    /// to different workspaces.
    ///
    /// # Arguments
    ///
    /// * `connection_ids` - The connection UUIDs to load
    ///
    /// # Returns
    ///
    /// A registry populated with the requested connections.
    pub async fn load_connections(&self, connection_ids: &[Uuid]) -> Result<ConnectionRegistry> {
        let mut registry = ConnectionRegistry::new();

        for &connection_id in connection_ids {
            let provider_conn = self.load_connection(connection_id).await?;
            registry.register(connection_id, provider_conn);
        }

        Ok(registry)
    }
}

impl std::fmt::Debug for PgConnectionLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgConnectionLoader")
            .field("client", &"<PgClient>")
            .field("master_key", &"<REDACTED>")
            .finish()
    }
}

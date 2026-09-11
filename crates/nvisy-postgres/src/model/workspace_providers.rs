//! Workspace provider model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_providers;
use crate::types::ProviderType;

/// Workspace provider model: an encrypted inference-provider connection.
///
/// A provider stores encrypted credentials for an inference service the platform
/// calls (an LLM, an NER model); the concrete `provider` and its `provider_type`
/// (the model kind) distinguish them. A separate resource from a connection: no
/// syncs, no schedule.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_providers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceProvider {
    /// Unique provider identifier.
    pub id: Uuid,
    /// Reference to the workspace this provider belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this provider.
    pub account_id: Uuid,
    /// Human-readable provider display name.
    pub display_name: String,
    /// Provider identifier (`openai`, `ollama`, `anthropic`, ...).
    pub provider: String,
    /// Inference model type of the provider (llm, ner).
    pub provider_type: ProviderType,
    /// Encrypted provider config (XChaCha20-Poly1305 encrypted JSON):
    /// provider tag, credentials, and any provider-specific settings.
    pub encrypted_data: Vec<u8>,
    /// Whether the provider is enabled.
    pub is_active: bool,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: JsonValue,
    /// Timestamp when the provider was created.
    pub created_at: Timestamp,
    /// Timestamp when the provider was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the provider was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace provider.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_providers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceProvider {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Provider display name.
    pub display_name: String,
    /// Provider identifier, for indexing and filtering.
    pub provider: String,
    /// Inference model type of the provider.
    pub provider_type: ProviderType,
    /// Encrypted provider config.
    pub encrypted_data: Vec<u8>,
    /// Whether the provider is enabled.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

impl NewWorkspaceProvider {
    /// A minimal active OpenAI-LLM-style provider for `workspace_id`, for tests.
    ///
    /// The display name is unique per call, so several test providers can be
    /// seeded into one workspace without colliding on the per-workspace
    /// display-name unique index.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid, provider_type: ProviderType) -> Self {
        let hex = Uuid::now_v7().simple().to_string();
        let suffix = &hex[hex.len() - 12..];
        Self {
            workspace_id,
            account_id,
            display_name: format!("Test Provider {suffix}"),
            provider: "openai".to_owned(),
            provider_type,
            encrypted_data: vec![1, 2, 3],
            is_active: Some(true),
            metadata: None,
        }
    }
}

/// Data for updating a workspace provider.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_providers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceProvider {
    /// Provider display name.
    pub display_name: Option<String>,
    /// Provider identifier.
    pub provider: Option<String>,
    /// Encrypted provider config.
    pub encrypted_data: Option<Vec<u8>>,
    /// Whether the provider is enabled.
    pub is_active: Option<bool>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

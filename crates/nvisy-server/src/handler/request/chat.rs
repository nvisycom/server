//! Chat session request types.

use nvisy_postgres::model::{NewChatSession, UpdateChatSession as UpdateChatSessionModel};
use nvisy_postgres::types::ChatSessionStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new chat session.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSession {
    /// ID of the primary file being edited in this session.
    pub primary_file_id: Uuid,
    /// Display name of the session.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// LLM configuration (model, temperature, max tokens, etc.).
    pub model_config: Option<serde_json::Value>,
}

impl CreateChatSession {
    /// Converts this request into a database model.
    pub fn into_model(self, workspace_id: Uuid, account_id: Uuid) -> NewChatSession {
        NewChatSession {
            workspace_id,
            account_id,
            primary_file_id: self.primary_file_id,
            display_name: self.display_name,
            model_config: self.model_config,
            session_status: None,
        }
    }
}

/// Request payload for updating a chat session.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatSession {
    /// Updated display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Updated session status.
    pub session_status: Option<ChatSessionStatus>,
    /// Updated LLM configuration.
    pub model_config: Option<serde_json::Value>,
}

impl UpdateChatSession {
    /// Converts this request into a database model.
    pub fn into_model(self) -> UpdateChatSessionModel {
        UpdateChatSessionModel {
            display_name: self.display_name,
            session_status: self.session_status,
            model_config: self.model_config,
            ..Default::default()
        }
    }
}

/// Request payload for sending a chat message.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessage {
    /// The message content to send.
    #[validate(length(min = 1, max = 32000))]
    pub content: String,
    /// Optional model override for this message.
    pub model: Option<String>,
}

//! Assistant chat request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Path parameters for a chat session.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionPathParams {
    /// The session id.
    pub session_id: Uuid,
}

/// Request to create a chat session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSession {
    /// Optional title. Defaults to a title seeded from the first message.
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
}

/// Request to send a message and stream the assistant's reply.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessage {
    /// The user's message.
    #[validate(length(min = 1, max = 65536))]
    pub content: String,
}

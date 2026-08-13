//! Workspace webhook request types.
//!
//! This module provides request DTOs for workspace webhook management including
//! creation and updates.

use std::collections::HashMap;

use nvisy_postgres::model::{
    NewWorkspaceWebhook, UpdateWorkspaceWebhook as UpdateWorkspaceWebhookModel,
};
use nvisy_postgres::types::{Json, WebhookEvent, WebhookHeaders, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::handler::{ErrorKind, Result};

/// Request payload for creating a new workspace webhook.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhook {
    /// Human-readable name for the webhook (1-128 characters).
    #[validate(length(min = 1, max = 128))]
    pub display_name: String,
    /// Detailed description of the webhook's purpose (max 500 characters).
    #[validate(length(max = 500))]
    pub description: String,
    /// The URL to send webhook payloads to.
    #[validate(url, length(max = 2048))]
    pub url: String,
    /// List of event types this webhook should receive.
    pub events: Vec<WebhookEvent>,
    /// Optional custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Initial status of the webhook (enabled or disabled).
    pub status: Option<WebhookStatus>,
}

impl CreateWebhook {
    /// Converts this request into a [`NewWorkspaceWebhook`] model.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The workspace this webhook belongs to.
    /// * `account_id` - The account creating the webhook.
    #[inline]
    pub fn into_model(
        self,
        workspace_id: Uuid,
        account_id: Uuid,
        encrypted_secret: Vec<u8>,
    ) -> Result<NewWorkspaceWebhook> {
        let events = self.events.into_iter().map(Some).collect();
        let headers = validate_headers(self.headers)?;
        // Suspended is a system-only state; coerce a user-supplied Suspended to
        // Disabled (the user off-switch).
        let status = self.status.map(|s| match s {
            WebhookStatus::Suspended => WebhookStatus::Disabled,
            other => other,
        });

        Ok(NewWorkspaceWebhook {
            workspace_id,
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            events,
            headers,
            encrypted_secret,
            status,
            created_by: account_id,
        })
    }
}

/// Validates optional raw headers into a stored column, rejecting malformed names
/// or values with a `400`.
fn validate_headers(
    headers: Option<HashMap<String, String>>,
) -> Result<Option<Json<WebhookHeaders>>> {
    let Some(headers) = headers else {
        return Ok(None);
    };
    let headers = WebhookHeaders::try_new(headers).map_err(|err| {
        ErrorKind::BadRequest
            .with_message("Invalid webhook header")
            .with_context(err.to_string())
    })?;
    Ok(headers.into_column())
}

/// Request payload for updating an existing workspace webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebhook {
    /// Updated human-readable name for the webhook (1-128 characters).
    #[validate(length(min = 1, max = 128))]
    pub display_name: Option<String>,
    /// Updated description of the webhook's purpose (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Updated URL to send webhook payloads to.
    #[validate(url, length(max = 2048))]
    pub url: Option<String>,
    /// Updated list of event types this webhook should receive.
    pub events: Option<Vec<WebhookEvent>>,
    /// Updated custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Updated status (enabled or disabled). Ignored while the webhook is
    /// system-suspended.
    pub status: Option<WebhookStatus>,
}

impl UpdateWebhook {
    /// Converts this request into an [`UpdateWorkspaceWebhookModel`].
    ///
    /// While `current_status` is `Suspended` (system-set), the status field is
    /// ignored. A user-supplied `Suspended` is coerced to `Disabled`.
    #[inline]
    pub fn into_model(self, current_status: WebhookStatus) -> Result<UpdateWorkspaceWebhookModel> {
        let events = self.events.map(|e| e.into_iter().map(Some).collect());
        let headers = validate_headers(self.headers)?;
        // A system-suspended webhook ignores user status changes; coerce a
        // user-supplied Suspended to Disabled.
        let status = if current_status.is_suspended() {
            None
        } else {
            self.status.map(|s| match s {
                WebhookStatus::Suspended => WebhookStatus::Disabled,
                other => other,
            })
        };

        Ok(UpdateWorkspaceWebhookModel {
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            events,
            headers,
            status,
            ..Default::default()
        })
    }
}

/// Request payload for testing a webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct TestWebhook {
    /// Optional custom payload to send in the test request.
    /// If not provided, a default test payload will be used.
    pub payload: Option<serde_json::Value>,
}

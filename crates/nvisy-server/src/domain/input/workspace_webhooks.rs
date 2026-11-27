//! Webhook service inputs.

use std::collections::HashMap;

use nvisy_postgres::model::{
    NewWorkspaceWebhook, UpdateWorkspaceWebhook as UpdateWorkspaceWebhookModel,
};
use nvisy_postgres::types::{Json, WebhookEvent, WebhookHeaders, WebhookStatus};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};

/// Input for creating a webhook. The signing secret is minted and encrypted by
/// the service; the caller supplies only the endpoint and its delivery config.
pub struct CreateWebhookInput {
    /// Human-readable name for the webhook.
    pub display_name: String,
    /// Detailed description of the webhook's purpose.
    pub description: String,
    /// The URL to send webhook payloads to.
    pub url: String,
    /// Event types this webhook should receive.
    pub events: Vec<WebhookEvent>,
    /// Optional custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Initial status of the webhook.
    pub status: Option<WebhookStatus>,
}

impl CreateWebhookInput {
    /// Builds a [`NewWorkspaceWebhook`] model, folding in the encrypted secret.
    ///
    /// Rejects malformed headers with a `400`. `Suspended` is a system-only state,
    /// so a caller-supplied `Suspended` is coerced to `Disabled`.
    ///
    /// # Errors
    ///
    /// - `BadRequest` if a supplied header name or value is malformed.
    pub fn into_model(
        self,
        workspace_id: Uuid,
        account_id: Uuid,
        encrypted_secret: Vec<u8>,
    ) -> Result<NewWorkspaceWebhook> {
        let events = self.events.into_iter().map(Some).collect();
        let headers = validate_headers(self.headers)?;
        let status = self.status.map(coerce_status);

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

/// Input for updating a webhook. Every field is optional; unset fields are left
/// unchanged.
pub struct UpdateWebhookInput {
    /// Updated human-readable name.
    pub display_name: Option<String>,
    /// Updated description.
    pub description: Option<String>,
    /// Updated URL to send payloads to.
    pub url: Option<String>,
    /// Updated event types.
    pub events: Option<Vec<WebhookEvent>>,
    /// Updated custom headers.
    pub headers: Option<HashMap<String, String>>,
    /// Updated status. Re-enables a system-suspended webhook.
    pub status: Option<WebhookStatus>,
}

impl UpdateWebhookInput {
    /// Builds an [`UpdateWorkspaceWebhookModel`].
    ///
    /// A caller-supplied status always applies, so a system-suspended webhook is
    /// re-enabled by setting `Enabled`; `Suspended` itself is system-only and is
    /// coerced to `Disabled`. Setting a status restarts the failure streak, so a
    /// re-enabled webhook is not suspended again by its next single failure.
    ///
    /// # Errors
    ///
    /// - `BadRequest` if a supplied header name or value is malformed.
    pub fn into_model(self) -> Result<UpdateWorkspaceWebhookModel> {
        let events = self.events.map(|e| e.into_iter().map(Some).collect());
        let headers = validate_headers(self.headers)?;
        let status = self.status.map(coerce_status);
        let consecutive_failures = status.map(|_| 0);

        Ok(UpdateWorkspaceWebhookModel {
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            events,
            headers,
            status,
            consecutive_failures,
            ..Default::default()
        })
    }
}

/// Coerces a caller-supplied status: `Suspended` is a system-only state, so it
/// maps to the user off-switch `Disabled`.
fn coerce_status(status: WebhookStatus) -> WebhookStatus {
    match status {
        WebhookStatus::Suspended => WebhookStatus::Disabled,
        other => other,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn update(status: Option<WebhookStatus>) -> UpdateWebhookInput {
        UpdateWebhookInput {
            display_name: None,
            description: None,
            url: None,
            events: None,
            headers: None,
            status,
        }
    }

    #[test]
    fn setting_a_status_restarts_the_failure_streak() {
        let enabled = update(Some(WebhookStatus::Enabled)).into_model().unwrap();
        assert_eq!(enabled.status, Some(WebhookStatus::Enabled));
        assert_eq!(enabled.consecutive_failures, Some(0));

        // Suspension is system-only: a caller asking for it gets the off switch.
        let suspended = update(Some(WebhookStatus::Suspended)).into_model().unwrap();
        assert_eq!(suspended.status, Some(WebhookStatus::Disabled));

        let unchanged = update(None).into_model().unwrap();
        assert_eq!(unchanged.status, None);
        assert_eq!(unchanged.consecutive_failures, None);
    }
}

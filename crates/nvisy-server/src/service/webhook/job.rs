//! The webhook delivery job carried over NATS.

use nvisy_postgres::types::WebhookEvent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A webhook delivery job enqueued on the NATS `WEBHOOKS` stream.
///
/// The job is deliberately slim: it identifies the webhook and the event, but
/// carries no endpoint URL, headers, or signing secret. The worker loads those
/// from the webhook's current configuration at delivery time, so an edited or
/// disabled webhook is honored even for jobs already in the queue, and the
/// plaintext signing secret never travels over — or rests in — NATS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookJob {
    /// The webhook to deliver to.
    pub webhook_id: Uuid,
    /// The workspace the webhook belongs to (the decryption key scope).
    pub workspace_id: Uuid,
    /// The event that triggered the delivery.
    pub event: WebhookEvent,
    /// The resource the event concerns.
    pub resource_id: Uuid,
    /// The account that triggered the event, if any.
    pub triggered_by: Option<Uuid>,
    /// Event-specific payload data.
    pub data: Option<serde_json::Value>,
}

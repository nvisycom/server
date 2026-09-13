//! Webhook service outputs.

use nvisy_postgres::model::WorkspaceWebhook;

/// A created webhook paired with its plaintext signing secret.
///
/// The secret is returned once, so the caller can surface it a single time; only
/// its encrypted form is stored.
pub struct CreatedWebhook {
    /// The webhook row.
    pub webhook: WorkspaceWebhook,
    /// The plaintext signing secret, shown once at creation.
    pub secret: String,
}

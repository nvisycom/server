//! Webhook service wrapper and health types.

mod health;
mod service;

pub use health::ServiceHealth;
pub use service::WebhookService;

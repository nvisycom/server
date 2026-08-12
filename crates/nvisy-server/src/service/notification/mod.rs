//! In-app notification emission.
//!
//! The request-side counterpart to the notification read API: creates
//! per-account notification rows for domain events. Unlike webhooks (which fan
//! out to configured external endpoints via NATS), notifications target a
//! specific account and are written synchronously; the client reads them back
//! through the notification endpoints.

mod emitter;

pub use emitter::NotificationEmitter;

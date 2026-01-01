//! [`CustomRoutes`] and other utilities.

use nvisy_postgres::types::WebhookEvent;

mod custom_routes;
mod parse_headers;

pub use custom_routes::{CustomRoutes, RouterMapFn};
pub use parse_headers::{parse_headers, serialize_headers, serialize_headers_opt};

/// Converts `Vec<Option<WebhookEvent>>` to `Vec<WebhookEvent>`.
pub fn flatten_events(events: Vec<Option<WebhookEvent>>) -> Vec<WebhookEvent> {
    events.into_iter().flatten().collect()
}

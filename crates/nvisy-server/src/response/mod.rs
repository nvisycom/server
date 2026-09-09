//! Outbound response types — the `IntoResponse` side of the server, mirroring
//! axum's own split between [`extract`](crate::extract) (inbound) and response.
//!
//! Serializable payload DTOs live with their handlers in
//! [`handler::response`](crate::handler::response); this module is for types
//! whose job is response *behavior* — setting status, headers, or cookies.

mod auth;
mod avatar_image;
mod sse;

pub use auth::{ClearedSession, CookieConfig, WebSession};
pub use avatar_image::AvatarImage;
pub use sse::SseResponse;

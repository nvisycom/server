//! Outbound response types — the `IntoResponse` side of the server, mirroring
//! axum's own split between [`extract`](crate::extract) (inbound) and response.
//!
//! Serializable payload DTOs live with their handlers in
//! [`handler::response`](crate::handler::response); this module is for types
//! whose job is response *behavior* — setting status, headers, or cookies.

mod session;

pub use session::{ClearedSession, CookieConfig, WebSession};

//! Request ID generation and propagation utilities.

use tower_http::request_id::MakeRequestUuid;

/// Default request ID generator using UUIDs.
///
/// This generates a unique UUID v4 for each request and sets it
/// as the `x-request-id` header.
pub type DefaultRequestIdMaker = MakeRequestUuid;

//! Observability middleware for monitoring and debugging.
//!
//! This module provides middleware for:
//! - Request metrics and performance tracking
//! - Distributed tracing with request IDs
//! - Structured logging

mod metrics;
mod request_id;
mod tracing;

pub(crate) use metrics::track_categorized_metrics;
pub use tracing::{
    create_propagate_request_id_layer, create_request_id_layer, create_sensitive_headers_layer,
    create_trace_layer,
};

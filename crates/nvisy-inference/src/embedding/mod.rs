//! Embedding generation types and operations.
//!
//! This module provides types for text and document embedding generation,
//! supporting both single and batch operations.

mod request;
mod response;

pub use request::{EmbeddingBatchRequest, EmbeddingRequest};
pub use response::{EmbeddingBatchResponse, EmbeddingFormat, EmbeddingResponse};

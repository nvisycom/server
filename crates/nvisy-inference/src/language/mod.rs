//! Vision-language model (VLM) types and operations.
//!
//! This module provides types for vision-language model inference,
//! supporting both single and batch operations with image and text inputs.

mod request;
mod response;

pub use request::{VlmBatchRequest, VlmRequest};
pub use response::{VlmBatchResponse, VlmResponse, VlmUsage};

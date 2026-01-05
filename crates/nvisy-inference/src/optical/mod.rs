//! Optical character recognition (OCR) types and operations.
//!
//! This module provides types for text extraction from images and documents,
//! supporting both single and batch operations.

mod request;
mod response;

pub use request::{OcrBatchRequest, OcrRequest};
pub use response::{OcrBatchResponse, OcrResponse, TextExtraction};

//! Optical character recognition (OCR) types and operations.
//!
//! This module provides types for text extraction from images and documents,
//! supporting both single and batch operations.

mod request;
mod response;

pub use request::{OcrBatchRequest, OcrRequest};
pub use response::{OcrBatchResponse, OcrResponse, TextExtraction};

use crate::Result;
use crate::service::Context;

/// Provider trait for optical character recognition.
///
/// Implement this trait to create custom OCR providers.
#[async_trait::async_trait]
pub trait OpticalProvider: Send + Sync {
    /// Process an image or document with OCR to extract text.
    async fn process_ocr(&self, context: &Context, request: &OcrRequest) -> Result<OcrResponse>;
}

/// Extension trait for batch OCR operations.
///
/// Provides default implementations for batch processing.
#[async_trait::async_trait]
pub trait OpticalProviderExt: OpticalProvider {
    /// Process a batch of OCR requests.
    ///
    /// The default implementation processes requests concurrently.
    async fn process_ocr_batch(
        &self,
        context: &Context,
        request: &OcrBatchRequest,
    ) -> Result<OcrBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.process_ocr(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(OcrBatchResponse::new(responses))
    }
}

// Blanket implementation for all OpticalProvider implementations
impl<T: OpticalProvider + ?Sized> OpticalProviderExt for T {}

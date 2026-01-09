//! Vision-language model (VLM) types and operations.
//!
//! This module provides types for vision-language model inference,
//! supporting both single and batch operations with image and text inputs.

mod request;
mod response;

pub use request::{VlmBatchRequest, VlmRequest};
pub use response::{VlmBatchResponse, VlmResponse};

use crate::Result;
use crate::service::Context;

/// Provider trait for language model operations.
///
/// Implement this trait to create custom language model providers.
#[async_trait::async_trait]
pub trait LanguageProvider: Send + Sync {
    /// Process a vision-language request and return a response.
    async fn process_vlm(&self, context: &Context, request: &VlmRequest) -> Result<VlmResponse>;
}

/// Extension trait for batch language model operations.
///
/// Provides default implementations for batch processing.
#[async_trait::async_trait]
pub trait LanguageProviderExt: LanguageProvider {
    /// Process a batch of VLM requests.
    ///
    /// The default implementation processes requests concurrently.
    async fn process_vlm_batch(
        &self,
        context: &Context,
        request: &VlmBatchRequest,
    ) -> Result<VlmBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.process_vlm(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(VlmBatchResponse::new(responses))
    }
}

// Blanket implementation for all LanguageProvider implementations
impl<T: LanguageProvider + ?Sized> LanguageProviderExt for T {}

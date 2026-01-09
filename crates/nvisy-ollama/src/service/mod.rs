//! InferenceProvider implementation for Ollama.

mod embedding;
mod language;
mod optical;

use nvisy_inference::types::ServiceHealth;
use nvisy_inference::{
    Context, EmbeddingProvider, InferenceProvider, LanguageProvider, OpticalProvider,
};

use crate::OllamaClient;

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaClient {
    async fn generate_embedding(
        &self,
        context: &Context,
        request: &nvisy_inference::EmbeddingRequest,
    ) -> nvisy_inference::Result<nvisy_inference::EmbeddingResponse> {
        embedding::generate(self, context, request).await
    }
}

#[async_trait::async_trait]
impl OpticalProvider for OllamaClient {
    async fn process_ocr(
        &self,
        context: &Context,
        request: &nvisy_inference::OcrRequest,
    ) -> nvisy_inference::Result<nvisy_inference::OcrResponse> {
        optical::process(self, context, request).await
    }
}

#[async_trait::async_trait]
impl LanguageProvider for OllamaClient {
    async fn process_vlm(
        &self,
        context: &Context,
        request: &nvisy_inference::VlmRequest,
    ) -> nvisy_inference::Result<nvisy_inference::VlmResponse> {
        language::process(self, context, request).await
    }
}

#[async_trait::async_trait]
impl InferenceProvider for OllamaClient {
    async fn health_check(&self) -> nvisy_inference::Result<ServiceHealth> {
        self.health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| nvisy_inference::Error::external_error().with_message(e.to_string()))
    }
}

//! Mock embedding provider for testing.

#[cfg(feature = "config")]
use clap::Args;
use nvisy_core::emb::{EmbeddingProvider, Request, Response};
use nvisy_core::{Result, ServiceHealth};
use serde::{Deserialize, Serialize};

/// Configuration for the mock embedding provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct MockEmbeddingConfig {
    // Empty for now, but can be extended with configuration options
}

/// Mock embedding provider for testing.
///
/// Returns default responses for all embedding requests.
#[derive(Clone, Default, Debug)]
pub struct MockEmbeddingProvider {
    #[allow(dead_code)]
    config: MockEmbeddingConfig,
}

impl MockEmbeddingProvider {
    /// Creates a new mock embedding provider with the given configuration.
    pub fn new(config: MockEmbeddingConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl<Req, Resp> EmbeddingProvider<Req, Resp> for MockEmbeddingProvider
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + Default + 'static,
{
    async fn generate_embedding(&self, request: Request<Req>) -> Result<Response<Resp>> {
        Ok(Response::new(request.request_id, Resp::default()))
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}

//! Mock VLM provider for testing.

use std::time::{Instant, SystemTime};

#[cfg(feature = "config")]
use clap::Args;
use jiff::SignedDuration;
use nvisy_core::vlm::{BoxedStream, Request, Response, VlmProvider};
use nvisy_core::{Result, ServiceHealth, SharedContext, UsageStats};
use serde::{Deserialize, Serialize};

/// Configuration for the mock VLM provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct MockLanguageConfig {
    // Empty for now, but can be extended with configuration options
}

/// Mock VLM provider for testing.
///
/// Returns default responses for all VLM requests.
#[derive(Clone, Default, Debug)]
pub struct MockLanguageProvider {
    #[allow(dead_code)]
    config: MockLanguageConfig,
}

impl MockLanguageProvider {
    /// Creates a new mock VLM provider with the given configuration.
    pub fn new(config: MockLanguageConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl<Req, Resp> VlmProvider<Req, Resp> for MockLanguageProvider
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + Default + 'static,
{
    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &Request<Req>,
    ) -> Result<Response<Resp>> {
        let start = Instant::now();

        let response = Response {
            content: "Mock VLM response".to_string(),
            usage: None,
            finish_reason: Some("stop".to_string()),
            created: SystemTime::now(),
            confidence: Some(0.95),
            visual_analysis: None,
            metadata: Default::default(),
            payload: Resp::default(),
        };

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        let runs = request.images.len() as u32;
        let tokens = response.content.len() as u32 / 4; // Rough token estimate
        context
            .record(UsageStats::success(tokens, runs, processing_time))
            .await;

        Ok(response)
    }

    async fn process_vlm_stream(
        &self,
        _context: &SharedContext,
        _request: &Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        Ok(Box::new(futures::stream::empty()))
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}

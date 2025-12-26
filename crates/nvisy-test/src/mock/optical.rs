//! Mock OCR provider for testing.

use std::time::Instant;

#[cfg(feature = "config")]
use clap::Args;
use jiff::SignedDuration;
use nvisy_core::ocr::{BoxedStream, OcrProvider, Request, Response};
use nvisy_core::{Result, ServiceHealth, SharedContext, UsageStats};
use serde::{Deserialize, Serialize};

/// Configuration for the mock OCR provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct MockOpticalConfig {
    // Empty for now, but can be extended with configuration options
}

/// Mock OCR provider for testing.
///
/// Returns default responses for all OCR requests.
#[derive(Clone, Default, Debug)]
pub struct MockOpticalProvider {
    #[allow(dead_code)]
    config: MockOpticalConfig,
}

impl MockOpticalProvider {
    /// Creates a new mock OCR provider with the given configuration.
    pub fn new(config: MockOpticalConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl<Req, Resp> OcrProvider<Req, Resp> for MockOpticalProvider
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + Default + 'static,
{
    async fn process_ocr(
        &self,
        context: &SharedContext,
        request: Request<Req>,
    ) -> Result<Response<Resp>> {
        let start = Instant::now();

        let response = Response::new(request.request_id, Resp::default());

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        let runs = 1u32; // Mock assumes 1 page per request
        context
            .record(UsageStats::success(0, runs, processing_time))
            .await;

        Ok(response)
    }

    async fn process_ocr_stream(
        &self,
        _context: &SharedContext,
        _request: Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        Ok(Box::new(futures::stream::empty()))
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}

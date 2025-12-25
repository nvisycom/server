//! Mock OCR provider for testing.

#[cfg(feature = "config")]
use clap::Args;
use nvisy_core::ocr::{BoxedStream, OcrProvider, Request, Response};
use nvisy_core::{Result, ServiceHealth};
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
    async fn process_ocr(&self, request: Request<Req>) -> Result<Response<Resp>> {
        Ok(Response::new(request.request_id, Resp::default()))
    }

    async fn process_ocr_stream(
        &self,
        _request: Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        Ok(Box::new(futures::stream::empty()))
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}

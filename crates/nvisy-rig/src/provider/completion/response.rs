//! Response types for completion provider.

use rig::completion::{GetTokenUsage, Usage};
use serde::{Deserialize, Serialize};

/// Unified raw response type for CompletionProvider.
///
/// This type normalizes responses from different providers into a common format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// The provider name.
    pub provider: String,
    /// The model name used.
    pub model: String,
}

impl GetTokenUsage for ProviderResponse {
    fn token_usage(&self) -> Option<Usage> {
        None
    }
}

/// Streaming response placeholder for CompletionProvider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStreamingResponse {
    /// The provider name.
    pub provider: String,
    /// The model name used.
    pub model: String,
    /// Token usage if available.
    pub usage: Option<Usage>,
}

impl GetTokenUsage for ProviderStreamingResponse {
    fn token_usage(&self) -> Option<Usage> {
        self.usage
    }
}

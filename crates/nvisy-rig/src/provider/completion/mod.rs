//! Completion models and providers.

mod model;
mod provider;
mod response;
mod rig_impl;

pub use model::{
    AnthropicModel, CohereCompletionModel, CompletionModel, GeminiCompletionModel,
    OpenAiCompletionModel, PerplexityModel,
};
pub use provider::CompletionProvider;
#[allow(unused_imports)]
pub use response::{ProviderResponse, ProviderStreamingResponse};

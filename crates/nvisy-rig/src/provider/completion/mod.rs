//! Completion models and providers.

mod credentials;
mod model;
mod provider;
mod response;
mod rig_impl;

pub use credentials::CompletionCredentials;
pub use model::{
    AnthropicModel, CohereCompletionModel, CompletionModel, GeminiCompletionModel,
    OpenAiCompletionModel, PerplexityModel,
};
pub use provider::CompletionProvider;
// Response types are part of the public API for CompletionModel trait consumers
#[allow(unused_imports)]
pub use response::{ProviderResponse, ProviderStreamingResponse};

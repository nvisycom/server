//! The client layer: the typed connection config and the runtime assistant
//! client.
//!
//! [`LlmConfig`] is the stored, provider-tagged configuration; [`AssistantClient`]
//! is the provider-agnostic runtime handle it builds.

mod config;
mod service;
mod token_stream;
mod turn;

pub use self::config::{
    AssistantConfig, AuthenticatedProvider, LlmConfig, UnauthenticatedProvider,
};
pub use self::service::AssistantClient;
pub use self::token_stream::TokenStream;
pub use self::turn::{ChatTurn, Role};

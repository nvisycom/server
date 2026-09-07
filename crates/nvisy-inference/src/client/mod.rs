//! The client layer: the typed connection config and the runtime inference
//! client.
//!
//! [`LlmConfig`] is the stored, provider-tagged configuration; [`InferenceClient`]
//! is the provider-agnostic runtime handle it builds.

mod config;
mod service;
mod token_stream;
mod turn;

pub use self::config::{AuthenticatedProvider, LlmConfig, UnauthenticatedProvider};
pub use self::service::InferenceClient;
pub use self::token_stream::TokenStream;
pub use self::turn::{ChatTurn, Role};

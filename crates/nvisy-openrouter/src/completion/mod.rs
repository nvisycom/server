//! Typed chat completion functionality.
//!
//! This module provides typed request/response handling for LLM completions,
//! including specialized redaction operations.

pub mod chat_completion;
pub mod chat_request;
pub mod chat_response;
pub mod redaction;
pub mod redaction_categories;
pub mod redaction_prompts;
pub mod redaction_request;
pub mod redaction_response;

pub use chat_completion::TypedChatCompletion;
pub use chat_request::{TypedChatRequest, TypedChatRequestBuilder, TypedChatRequestBuilderError};
pub use chat_response::{
    TypedChatResponse, TypedChatResponseBuilder, TypedChatResponseBuilderError,
};
pub use redaction::RedactionService;
pub use redaction_categories::RedactionCategory;
pub use redaction_request::{
    RedactionItem, RedactionItemBuilder, RedactionRequest, RedactionRequestBuilder,
};
pub use redaction_response::{
    Entity, EntityBuilder, RedactedData, RedactedDataBuilder, RedactionResponse,
    RedactionResponseBuilder,
};

//! Typed chat completion functionality.
//!
//! This module provides typed request/response handling for LLM completions with
//! automatic JSON Schema generation and structured output parsing using portkey-sdk 0.2.
//!
//! # Examples
//!
//! ## Basic Structured Output
//!
//! ```rust,no_run
//! use nvisy_portkey::{LlmClient, completion::{ChatCompletion, ChatContext}};
//! use portkey_sdk::model::{ChatCompletionRequest, ResponseFormat};
//! use serde::{Serialize, Deserialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema, Debug)]
//! struct MovieRecommendation {
//!     title: String,
//!     year: u16,
//!     rating: f32,
//!     genre: String,
//!     reason: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = LlmClient::from_api_key("your-api-key")?;
//! let mut context = ChatContext::new("You are a movie expert");
//! context.add_user_message("Recommend a great sci-fi movie from the 1980s");
//!
//! let mut request = ChatCompletionRequest::new(
//!     "gpt-4o",
//!     context.to_messages()
//! );
//!
//! // Configure structured output using JSON Schema
//! request.response_format = Some(ResponseFormat::JsonSchema {
//!     json_schema: ResponseFormat::json_schema::<MovieRecommendation>()
//!         .with_description("A movie recommendation with details")
//!         .with_strict(true),
//! });
//!
//! let response = client.structured_chat_completion::<MovieRecommendation>(
//!     &mut context,
//!     request
//! ).await?;
//!
//! if let Some(movie) = response {
//!     println!("🎬 Movie: {} ({})", movie.title, movie.year);
//!     println!("   Genre: {}", movie.genre);
//!     println!("   Rating: {:.1}/10", movie.rating);
//!     println!("   Reason: {}", movie.reason);
//! }
//! # Ok(())
//! # }
//! ```

pub mod chat_completion;
pub mod chat_context;
pub mod chat_request;
pub mod chat_response;
pub mod redaction;
pub mod redaction_categories;
pub mod redaction_prompts;
pub mod redaction_request;
pub mod redaction_response;

pub use chat_completion::ChatCompletion;
pub use chat_context::ChatContext;
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

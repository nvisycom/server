//! OpenRouter API service client and configuration.
//!
//! This module provides a unified client for interacting with OpenRouter's API,
//! including chat completions, model information, rate limiting, and health monitoring.
//!
//! # Examples
//!
//! ```rust,no_run
//! use nvisy_openrouter::{LlmClient, LlmConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = LlmClient::from_api_key("your-api-key")?;
//!     let response = client.chat_completion("Hello, world!").await?;
//!     println!("Response: {}", response.choices[0].message.content);
//!     Ok(())
//! }
//! ```

pub use self::client::LlmClient;
pub use self::config::LlmConfig;
pub use self::error::{Error, Result};

pub mod client;
pub mod component;
pub mod config;
pub mod error;

// Re-export commonly used types from openrouter_api for convenience
pub use openrouter_api::{ChatCompletionRequest, ChatCompletionResponse, Message, ModelInfo};

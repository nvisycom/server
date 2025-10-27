//! # nvisy-openrouter
//!
//! A high-level, production-ready client for OpenRouter's API with comprehensive
//! error handling, rate limiting, observability, and data redaction capabilities.
//!
//! This crate provides both service client functionality and specialized data redaction
//! utilities, making it easy to integrate OpenRouter's AI models for privacy-focused
//! data processing tasks.
//!
//! ## Features
//!
//! - **Service Client**: Full-featured OpenRouter API client with rate limiting
//! - **Error Handling**: Comprehensive error types with recovery strategies
//! - **Health Monitoring**: Built-in health checks and component status reporting
//! - **Data Redaction**: Specialized tools for identifying sensitive data to redact
//! - **JSON Processing**: Structured input/output format for redaction tasks
//! - **Observability**: Structured logging and metrics integration
//!
//! ## Quick Start
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use nvisy_openrouter::{RedactionClient, RedactionRequest, RedactionItem, LlmClient};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create clients
//!     let llm_client = LlmClient::from_api_key("your-openrouter-api-key")?;
//!     let redaction_client = RedactionClient::new(llm_client);
//!
//!     // Create redaction request
//!     let request = RedactionRequest {
//!         data: vec![
//!             RedactionItem {
//!                 id: "1".to_string(),
//!                 text: "John lives at 123 Main St".to_string(),
//!                 entity: "John Doe".to_string(),
//!                 data_type: "address".to_string(),
//!             }
//!         ],
//!         prompt: "Redact personal addresses".to_string(),
//!     };
//!
//!     let response = redaction_client.redact(&request).await?;
//!     println!("Redact IDs: {:?}", response.redact_ids);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Advanced Configuration
//!
//! ```rust,no_run
//! use nvisy_openrouter::{LlmClient, LlmConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a custom configuration
//!     let config = LlmConfig::new()
//!         .with_rate_limit(20.try_into().unwrap()) // 20 requests per second
//!         .with_default_model("anthropic/claude-3-haiku")
//!         .with_tracing(true);
//!
//!     let client = LlmClient::from_api_key_with_config("your-api-key", config)?;
//!
//!     // Use the client...
//!     Ok(())
//! }
//! ```
//!
//! ### Using Data Redaction
//!
//! ```rust,no_run
//! use nvisy_openrouter::{LlmClient, RedactionClient, RedactionRequest, RedactionItem};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let llm_client = LlmClient::from_api_key("your-api-key")?;
//!     let redaction_client = RedactionClient::new(llm_client);
//!
//!     // Create a redaction request
//!     let request = RedactionRequest {
//!         data: vec![
//!             RedactionItem {
//!                 id: "1".to_string(),
//!                 text: "123 Main St, 555-1234".to_string(),
//!                 entity: "John Doe".to_string(),
//!                 data_type: "address".to_string(),
//!             },
//!             RedactionItem {
//!                 id: "2".to_string(),
//!                 text: "8th of January, 1990".to_string(),
//!                 entity: "John Doe".to_string(),
//!                 data_type: "date of birth".to_string(),
//!             }
//!         ],
//!         prompt: "Redact all addresses that belong to John Doe".to_string(),
//!     };
//!
//!     let response = redaction_client.redact(&request).await?;
//!     println!("Items to redact: {:?}", response.redact_ids); // ["1"]
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Module Organization
//!
//! - [`service`] - OpenRouter API client, configuration, and health monitoring
//! - [`prompt`] - Data redaction utilities and structured processing
//!
//! ## Configuration
//!
//! The client uses a default configuration that is suitable for most applications.
//! You can customize the configuration using the builder pattern:
//!
//! ```rust,no_run
//! use nvisy_openrouter::{LlmClient, LlmConfig};
//!
//! let config = LlmConfig::new()
//!     .with_rate_limit(10.try_into().unwrap())
//!     .with_default_model("anthropic/claude-3-haiku")
//!     .with_tracing(true)
//!     .build()?;
//!
//! let client = LlmClient::from_api_key_with_config("your-api-key", config)?;
//! # Ok::<(), nvisy_openrouter::Error>(())
//! ```
//!
//! ## Error Handling
//!
//! The crate provides comprehensive error types that help you handle different
//! failure scenarios appropriately:
//!
//! ```rust,no_run
//! use nvisy_openrouter::{LlmClient, Error};
//!
//! async fn handle_request(client: &LlmClient) -> Result<String, Error> {
//!     match client.chat_completion("Hello").await {
//!         Ok(response) => Ok(response.choices[0].message.content.clone()),
//!         Err(Error::RateLimit { retry_after, .. }) => {
//!             if let Some(delay) = retry_after {
//!                 tokio::time::sleep(delay).await;
//!                 // Retry the request
//!             }
//!             Err(Error::rate_limit("Rate limit exceeded"))
//!         }
//!         Err(Error::Auth { .. }) => {
//!             // Handle authentication error
//!             Err(Error::auth("Invalid API key"))
//!         }
//!         Err(other) => Err(other),
//!     }
//! }
//! ```
//!
//! ## Health Monitoring
//!
//! The client implements the [`nvisy_error::Component`] trait for health monitoring:
//!
//! ```rust,no_run
//! use nvisy_openrouter::LlmClient;
//! use nvisy_error::Component;
//!
//! async fn check_service_health(client: &LlmClient) {
//!     let status = client.current_status().await;
//!     println!("Service status: {:?}", status.health_status);
//! }
//! ```

/// Logging target for OpenRouter client operations.
pub const OPENROUTER_TARGET: &str = "nvisy::service::openrouter";

// // Re-export the main modules
// pub mod prompt;
// pub mod service;

// // Re-export commonly used types for convenience
// // Re-export redaction prompt utilities
// pub use prompt::{
//     RedactionClient, RedactionItem, RedactionPrompt, RedactionRequest, RedactionResponse,
// };
// // Re-export OpenRouter API types that users commonly need
// pub use service::{ChatCompletionRequest, ChatCompletionResponse, Message, ModelInfo};
// pub use service::{Error, LlmClient, LlmConfig, Result};

// // Type alias for backward compatibility
// pub type OpenRouter = LlmClient;

// #[cfg(test)]
// mod integration_tests {
//     use super::*;

//     #[test]
//     fn test_public_api_availability() {
//         // Test that all main public types are accessible
//         let _config = LlmConfig::default();
//         let _redaction_prompt = RedactionPrompt::new();

//         // Test redaction types
//         let _item = RedactionItem {
//             id: "1".to_string(),
//             text: "test".to_string(),
//             entity: "test_entity".to_string(),
//             data_type: "test_type".to_string(),
//         };

//         let _request = RedactionRequest {
//             data: vec![],
//             prompt: "test prompt".to_string(),
//         };

//         let _response = RedactionResponse { redact_ids: vec![] };
//     }

//     #[test]
//     fn test_error_types() {
//         // Test that error types are properly exposed
//         let error = Error::config("Test error");
//         assert!(error.is_config_error());

//         let error = Error::rate_limit("Rate limit");
//         assert!(error.is_rate_limit_error());

//         let error = Error::auth("Auth error");
//         assert!(error.is_auth_error());
//     }

//     #[test]
//     fn test_redaction_integration() {
//         // Test that redaction utilities work together
//         let item = RedactionItem {
//             id: "1".to_string(),
//             text: "123 Main St".to_string(),
//             entity: "John Doe".to_string(),
//             data_type: "address".to_string(),
//         };

//         let request = RedactionRequest {
//             data: vec![item],
//             prompt: "Redact all addresses".to_string(),
//         };

//         assert_eq!(request.data.len(), 1);
//         assert_eq!(request.data[0].id, "1");
//         assert_eq!(request.data[0].entity, "John Doe");
//     }

//     #[test]
//     fn test_redaction_prompt_formatting() {
//         let prompt = RedactionPrompt::new();
//         let request = RedactionRequest {
//             data: vec![RedactionItem {
//                 id: "1".to_string(),
//                 text: "test data".to_string(),
//                 entity: "test entity".to_string(),
//                 data_type: "test type".to_string(),
//             }],
//             prompt: "test redaction".to_string(),
//         };

//         let formatted = prompt.format_request(&request);
//         assert!(formatted.contains("test data"));
//         assert!(formatted.contains("test redaction"));
//     }
// }

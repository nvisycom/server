//! # nvisy-portkey
//!
//! A high-level, production-ready client for Portkey's AI Gateway with comprehensive
//! error handling, observability, and data redaction capabilities.
//!
//! This crate provides both client functionality and specialized data redaction
//! services, making it easy to integrate Portkey's AI gateway for privacy-focused
//! data processing tasks across 200+ AI providers.
//!
//! ## Features
//!
//! - **Client**: Full-featured Portkey AI Gateway client
//! - **Error Handling**: Comprehensive error types with recovery strategies
//! - **Data Redaction**: Specialized service for identifying sensitive data to redact
//! - **JSON Processing**: Structured input/output format for redaction tasks
//! - **Observability**: Structured logging and metrics integration
//!
//! ## Examples
//!
//! Creating a client with the builder pattern:
//!
//! ```no_run
//! # use nvisy_portkey::{LlmConfig, Result};
//! # fn example() -> Result<()> {
//! let client = LlmConfig::builder()
//!     .with_api_key("your-api-key")
//!     .with_virtual_key("your-virtual-key")
//!     .with_default_model("gpt-4")
//!     .build_client()?;
//! # Ok(())
//! # }
//! ```
//!
//! Creating a client with just an API key:
//!
//! ```no_run
//! # use nvisy_portkey::{LlmClient, Result};
//! # fn example() -> Result<()> {
//! let client = LlmClient::from_api_key("your-api-key")?;
//! # Ok(())
//! # }
//! ```
//!
//! Creating a client with API key and virtual key:
//!
//! ```no_run
//! # use nvisy_portkey::{LlmClient, Result};
//! # fn example() -> Result<()> {
//! let client = LlmClient::from_keys("your-api-key", "your-virtual-key")?;
//! # Ok(())
//! # }
//! ```

/// Logging target for Portkey client operations.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_portkey::client";

/// Logging target for configuration operations.
pub const TRACING_TARGET_CONFIG: &str = "nvisy_portkey::config";

/// Logging target for schema generation and validation.
pub const TRACING_TARGET_SCHEMA: &str = "nvisy_portkey::schema";

/// Logging target for completion operations.
pub const TRACING_TARGET_COMPLETION: &str = "nvisy_portkey::completion";

// Core modules
mod client;
pub mod completion;
pub mod error;
#[doc(hidden)]
pub mod prelude;
pub mod typed;

// Re-export client types
pub use client::{LlmBuilder, LlmBuilderError, LlmClient, LlmConfig};
// Re-export error types
pub use error::{Error, Result};

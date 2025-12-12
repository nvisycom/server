//! Prelude module for nvisy-portkey.
//!
//! This module re-exports the most commonly used types and traits from nvisy-portkey,
//! making it easy to import everything you need with a single `use` statement.
//!
//! # Example
//!
//! ```rust
//! use nvisy_portkey::prelude::*;
//!
//! # fn example() -> Result<()> {
//! let client = LlmClient::from_api_key("your-api-key")?;
//! # Ok(())
//! # }
//! ```

#[doc(inline)]
pub use crate::completion::{
    ChatCompletion, ChatContext, Entity, RedactedData, RedactionCategory, RedactionItem,
    RedactionRequest, RedactionResponse, RedactionService, TypedChatRequest, TypedChatResponse,
};
#[doc(inline)]
pub use crate::{Error, LlmClient, LlmConfig, Result};

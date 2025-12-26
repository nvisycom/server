//! Prelude for the nvisy-olmocr2 crate
//!
//! This module re-exports the most commonly used types and traits from the crate
//! to provide a convenient single import for users.

pub use crate::client::{OlemCredentials, OlmClient, OlmConfig};
pub use crate::error::{Error, Result};
pub use crate::provider::OlmOcrProvider;

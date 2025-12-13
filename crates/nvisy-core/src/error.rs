//! Common error type definitions.

use std::error::Error as StdError;

/// Type alias for boxed dynamic errors that can be sent across threads.
///
/// This type is commonly used as a source error in structured error types,
/// providing a way to wrap any error that implements the standard `Error` trait
/// while maintaining Send and Sync bounds for multi-threaded contexts.
pub type BoxedError = Box<dyn StdError + Send + Sync>;

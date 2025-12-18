//! Prelude module for nvisy-server.
//!
//! This module re-exports the most commonly used types and traits from nvisy-server,
//! making it easy to import everything you need with a single `use` statement.
//!
//! # Example
//!
//! ```rust
//! use nvisy_server::prelude::*;
//! ```

// Re-export extractor types
pub use crate::extract::*;
// Re-export middleware types
pub use crate::middleware::*;
// Re-export service types
pub use crate::service::*;

//! Foundational traits for data I/O and vector operations.
//!
//! This crate provides the core abstractions for:
//! - Data input/output operations (storage backends)
//! - Vector store operations (embeddings storage)
//! - Common types used across integrations

#![forbid(unsafe_code)]

mod error;
mod input;
mod output;
mod types;
mod vector;

pub use error::{DataError, DataErrorKind, DataResult};
pub use input::{DataInput, InputContext};
pub use output::{DataOutput, OutputContext};
pub use types::{Metadata, VectorData, VectorSearchResult};
pub use vector::{VectorContext, VectorOutput, VectorSearchOptions};

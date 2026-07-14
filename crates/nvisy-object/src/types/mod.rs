//! Inlined types from `nvisy-core` to keep this crate self-contained.

pub mod content_data;
pub mod content_source;
pub mod error;

pub use content_data::ContentData;
pub use content_source::ContentSource;
pub use error::Error;

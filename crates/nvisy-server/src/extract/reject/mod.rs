//! Enhanced request extractors with improved error handling and validation.
//!
//! This module provides custom Axum extractors that enhance the default functionality
//! with better error messages, validation, and type safety. These extractors are
//! designed to be drop-in replacements for their standard Axum counterparts while
//! providing additional features like detailed error context and automatic validation.

mod form_with_rej;
mod json_with_rej;
mod mutlipart_with_rej;
mod path_with_rej;
mod query_with_rej;
mod validated_json;

pub use self::form_with_rej::Form;
pub use self::json_with_rej::Json;
pub use self::mutlipart_with_rej::Multipart;
pub use self::path_with_rej::Path;
pub use self::query_with_rej::Query;
pub use self::validated_json::ValidateJson;

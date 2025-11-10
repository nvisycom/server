//! Enhanced request extractors with improved error handling and validation.
//!
//! This module provides custom Axum extractors that enhance the default functionality
//! with better error messages, validation, and type safety. These extractors are
//! designed to be drop-in replacements for their standard Axum counterparts while
//! providing additional features like detailed error context and automatic validation.

pub mod enhanced_form;
pub mod enhanced_json;
pub mod enhanced_path;
pub mod enhanced_query;
pub mod validated_json;

pub use self::enhanced_form::Form;
pub use self::enhanced_json::Json;
pub use self::enhanced_path::Path;
pub use self::enhanced_query::Query;
pub use self::validated_json::ValidateJson;

//! Validation extractor and shared field validators.
//!
//! [`ValidateJson`] deserializes a request body (via the [`Json`] extractor) and
//! then runs `garde::Validate` on it. The [`validators`] module holds custom
//! `garde` validators that DTO fields reference by name.
//!
//! [`Json`]: crate::extract::Json

mod validated_json;
pub mod validators;

pub use self::validated_json::ValidateJson;

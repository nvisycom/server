//! Enhanced request extractors with improved error handling and validation.
//!
//! This module provides custom Axum extractors that enhance the default functionality
//! with better error messages, validation, and type safety. These extractors are
//! designed to be drop-in replacements for their standard Axum counterparts while
//! providing additional features like detailed error context and automatic validation.
//!
//! # Philosophy
//!
//! The default Axum extractors provide basic functionality but often produce
//! generic error messages that are not helpful for API consumers or debugging.
//! This module addresses those limitations by:
//!
//! - **Detailed Error Messages**: Contextual error information for better debugging
//! - **Consistent Error Format**: All extractors use the same error response structure
//! - **Validation Integration**: Automatic validation with comprehensive error reporting
//! - **Type Safety**: Strong typing with compile-time guarantees
//!
//! # Extractors
//!
//! ## JSON Handling
//!
//! - [`Json`] - Enhanced JSON deserialization with better error messages
//! - [`ValidateJson`] - JSON extraction with automatic validation using the `validator` crate
//!
//! ## Path Parameters
//!
//! - [`Path`] - Path parameter extraction with detailed error context
//!
//! # Usage Patterns
//!
//! ## Simple JSON Extraction
//!
//! ```rust,ignore
//! use nvisy_server::extract::Json;
//!
//! async fn create_user(Json(user): Json<CreateUserRequest>) -> Result<impl IntoResponse> {
//!     // JSON is automatically deserialized with better error handling
//!     Ok(StatusCode::CREATED)
//! }
//! ```
//!
//! ## JSON with Validation
//!
//! ```rust,ignore
//! use nvisy_server::extract::ValidateJson;
//! use validator::Validate;
//!
//! #[derive(Deserialize, Validate)]
//! struct CreateUserRequest {
//!     #[validate(email)]
//!     email: String,
//!     #[validate(length(min = 8))]
//!     password: String,
//! }
//!
//! async fn create_user(ValidateJson(user): ValidateJson<CreateUserRequest>) -> Result<impl IntoResponse> {
//!     // JSON is deserialized AND validated automatically
//!     Ok(StatusCode::CREATED)
//! }
//! ```
//!
//! ## Path Parameters
//!
//! ```rust,ignore
//! use nvisy_server::extract::Path;
//!
//! #[derive(Deserialize)]
//! struct UserPath {
//!     user_id: Uuid,
//! }
//!
//! async fn get_user(Path(params): Path<UserPath>) -> Result<impl IntoResponse> {
//!     // Path parameters are extracted with detailed error messages
//!     let user = find_user(params.user_id).await?;
//!     Ok(Json(user))
//! }
//! ```

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

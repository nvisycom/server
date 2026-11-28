//! The HTTP error response: [`Error`], [`ErrorKind`], [`Result`], and the
//! serialized [`ErrorResponse`] body, plus the `From` conversions that turn each
//! infrastructure error into an [`Error`] at the request boundary.

mod assistant_error;
mod core_error;
mod crypto_error;
mod engine_error;
mod error_response;
mod http_error;
mod nats_error;
mod oidc_error;
mod pg_constraint;
mod pg_error;
mod s3_error;

pub use error_response::ErrorResponse;
pub use http_error::{Error, ErrorKind, Resource, Result, error_responses};

//! The HTTP error response: [`Error`], [`ErrorKind`], [`Result`], and the
//! serialized [`ErrorResponse`] body, plus the `From` conversions that turn each
//! infrastructure error into an [`Error`] at the request boundary.

mod crypto_error;
mod engine_error;
mod error_response;
mod file_service_error;
mod http_error;
mod inference_error;
mod nats_error;
mod object_error;
mod oidc_error;
mod pg_account;
mod pg_document;
mod pg_error;
mod pg_pipeline;
mod pg_workspace;
mod s3_error;
mod webhook_error;

pub use error_response::ErrorResponse;
pub use http_error::{Error, ErrorKind, Result};

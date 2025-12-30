//! [`Error`], [`ErrorKind`] and [`Result`].

mod http_error;
mod nats_error;
mod pg_account;
mod pg_document;
mod pg_error;
mod pg_workspace;

pub use http_error::{Error, ErrorKind, Result};

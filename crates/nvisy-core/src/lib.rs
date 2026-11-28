#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "cli")]
mod duration;
mod error;
pub mod health;
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub mod http;
pub mod net;
mod secret;
#[cfg(feature = "test_util")]
pub mod test_util;
mod transfer;

#[cfg(feature = "cli")]
pub use duration::parse_duration;
pub use error::{BoxedError, Error, ErrorKind, Result, kind_for_status};
pub use secret::{REDACTED, Secret};
pub use transfer::{ByteStream, FileUpload};

/// Tracing target for core operations.
pub const TRACING_TARGET: &str = "nvisy_core";

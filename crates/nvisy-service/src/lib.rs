#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
pub mod inference;
#[doc(hidden)]
pub mod prelude;
pub mod types;
pub mod webhook;

pub use error::{BoxedError, Error, ErrorKind, Result};

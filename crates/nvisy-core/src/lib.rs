#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod provider;
pub mod types;

pub use error::{BoxedError, Error, ErrorKind, Result};
pub use provider::IntoProvider;

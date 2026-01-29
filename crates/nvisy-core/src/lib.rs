#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "encryption")]
#[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
pub mod crypto;

pub mod fs;
pub mod io;
pub mod path;

mod common;
pub mod error;

#[doc(hidden)]
pub mod prelude;

pub use common::{Provider, ServiceHealth, ServiceStatus, Timing};
pub use error::{BoxedError, Error, ErrorKind, Result};

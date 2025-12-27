#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;

pub mod extract;
pub mod handler;
pub mod middleware;
#[doc(hidden)]
pub mod prelude;
pub mod service;
pub mod utility;

pub use crate::error::{BoxedError, Error, ErrorKind, Result};

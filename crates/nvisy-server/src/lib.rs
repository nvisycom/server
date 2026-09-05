#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod args;
mod error;

pub mod extract;
pub mod handler;
pub mod middleware;
pub mod service;

pub use crate::args::ServiceArgs;
pub use crate::error::{BoxedError, Error, ErrorKind, Result};

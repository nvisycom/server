#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod args;
mod error;

pub mod domain;
pub mod extract;
pub mod handler;
pub mod middleware;
pub mod response;
pub mod service;
pub mod worker;

pub use crate::args::ServiceArgs;
pub use crate::error::{BoxedError, Error, ErrorKind, Result};

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;

pub mod extract;
pub mod handler;
pub mod middleware;
pub mod service;
pub mod worker;

pub use crate::error::{BoxedError, Error, ErrorKind, Result};

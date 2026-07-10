#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;

pub mod extract;
pub mod handler;
pub mod middleware;
pub mod service;

pub use crate::error::{BoxedError, Error, ErrorKind, Result};

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod core;
mod error;
mod runtime;

pub mod provider;

pub use core::{DataInput, DataOutput, Provider, Resumable, contexts, datatypes, params, streams};

pub use error::{BoxError, Error, ErrorKind, Result};

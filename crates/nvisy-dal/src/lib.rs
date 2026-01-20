//! Data Abstraction Layer for workflow inputs and outputs.
//!
//! This crate provides a unified interface for reading and writing data
//! across various storage backends.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod core;
pub mod datatype;
pub mod provider;

mod error;

pub use core::{Context, DataInput, DataOutput, InputStream, ItemSink, ItemStream, OutputStream};

pub use datatype::{AnyDataValue, DataTypeId};
pub use error::{Error, ErrorKind, Result};
pub use provider::ProviderConfig;

/// Alias for backwards compatibility with nvisy-opendal.
pub type StorageError = Error;
/// Alias for backwards compatibility.
pub type StorageConfig = ProviderConfig;

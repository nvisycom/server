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

pub use core::{
    DataInput, DataOutput, InputStream, ItemSink, ItemStream, ObjectContext, OutputStream,
    RelationalContext, VectorContext,
};
pub use datatype::{AnyDataValue, DataTypeId};
pub use error::{BoxError, Error, ErrorKind, Result};

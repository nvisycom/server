//! Data Abstraction Layer for workflow inputs and outputs.
//!
//! This crate provides a unified interface for reading and writing data
//! across various storage backends.
//!
//! # Architecture
//!
//! The DAL is split into two parts:
//! - **Rust**: Streaming, observability, unified interface, server integration
//! - **Python**: Provider implementations, client libraries, external integrations

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod core;
pub mod datatype;
pub mod provider;

mod python;

mod error;

pub use core::{
    AnyContext, DataInput, DataOutput, InputStream, ItemSink, ItemStream, ObjectContext,
    OutputStream, Provider, RelationalContext, VectorContext,
};

pub use datatype::{AnyDataValue, DataType, Document, Embedding, Graph, Message, Object, Record};
pub use error::{BoxError, Error, ErrorKind, Result};

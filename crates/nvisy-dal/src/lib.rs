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

mod core;
mod error;
mod runtime;

pub mod provider;

pub mod contexts {
    //! Context types for pagination and filtering.
    pub use crate::core::contexts::*;
}

pub mod datatypes {
    //! Data types for storage operations.
    pub use crate::core::datatypes::*;
}

pub mod params {
    //! Parameter types for provider configuration.
    pub use crate::core::params::*;
}

pub mod streams {
    //! Stream types for data input/output.
    pub use crate::core::streams::*;
}

pub use core::{DataInput, DataOutput};

pub use error::{BoxError, Error, ErrorKind, Result};
pub use nvisy_core::Provider;
pub use provider::{AnyCredentials, AnyParams, AnyProvider};

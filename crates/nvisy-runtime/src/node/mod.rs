//! Node types for workflow graphs.
//!
//! This module provides the core node abstractions:
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Input, Transformer, Output)

mod data;
mod id;
pub mod input;
pub mod output;
pub mod provider;
pub mod transformer;

pub use data::NodeData;
pub use id::NodeId;
pub use input::InputNode;
pub use output::OutputNode;
pub use provider::{
    AzblobCredentials, AzblobParams, CredentialsRegistry, GcsCredentials, GcsParams,
    MysqlCredentials, MysqlParams, PostgresCredentials, PostgresParams, ProviderCredentials,
    ProviderParams, S3Credentials, S3Params,
};
pub use transformer::{TransformerConfig, TransformerNode};

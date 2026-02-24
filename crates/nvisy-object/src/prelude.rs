//! Convenience re-exports.

pub use crate::providers::Provider;
pub use crate::streams::{StreamSource, StreamTarget};
pub use crate::types::{ContentData, ContentSource, Error};

pub use crate::client::{GetOutput, ObjectStoreClient, PutOutput};
pub use crate::providers::{AzureProvider, GcsProvider, S3Provider};
pub use crate::streams::{ObjectReadStream, ObjectWriteStream};

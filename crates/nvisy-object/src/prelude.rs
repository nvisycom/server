//! Convenience re-exports.

pub use crate::client::{GetOutput, ObjectStoreClient, PutOutput};
pub use crate::providers::{AzureProvider, Client, GcsProvider, S3Provider};
pub use crate::streams::{ObjectReadStream, ObjectWriteStream, StreamSource, StreamTarget};
pub use crate::types::{ContentData, ContentSource, Error};

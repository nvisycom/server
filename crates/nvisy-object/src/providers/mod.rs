//! Client trait and object storage providers.

mod provider;
mod azure;
mod gcs;
mod s3;

pub use provider::Client;
pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
pub use s3::{S3Credentials, S3Provider};

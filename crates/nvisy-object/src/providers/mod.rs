//! Client trait and object storage providers.

mod azure;
mod gcs;
mod provider;
mod s3;

pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
pub use provider::Client;
pub use s3::{S3Credentials, S3Provider};

//! Shared HTTP helpers for the provider clients.

use futures::TryStreamExt;
use reqwest::Response;

use crate::client::ByteStream;
use crate::error::Error;

/// Adapts a response body into the provider-neutral [`ByteStream`], mapping any
/// stream error into the crate error type. Used by every provider's download.
pub fn response_stream(response: Response) -> ByteStream {
    Box::pin(response.bytes_stream().map_err(Error::from))
}

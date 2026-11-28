//! Result type for [`ObjectStoreClient::get_stream`].
//!
//! [`ObjectStoreClient::get_stream`]: super::ObjectStoreClient::get_stream

use nvisy_core::ByteStream;

/// An object opened for streaming: its size and its body.
#[must_use]
pub struct GetStream {
    /// The object's size in bytes, as reported by the store.
    pub size: u64,
    /// The object's bytes, streamed in chunks.
    pub body: ByteStream,
}

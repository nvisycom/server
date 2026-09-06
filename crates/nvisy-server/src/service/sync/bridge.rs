//! Adapters between provider byte streams and `AsyncRead`.
//!
//! A [`FileSource`](super::file_source::FileSource) yields
//! `Stream<Item = Result<Bytes, _>>` while the first-party blob store consumes
//! and produces `AsyncRead`. These helpers convert between the two so an object
//! can be piped end to end without buffering the whole body in memory.

use std::io;

use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use tokio::io::AsyncRead;
use tokio_util::io::{ReaderStream, StreamReader};

/// Adapts a byte stream into an [`AsyncRead`].
///
/// Stream errors surface as [`io::Error`], as required by [`StreamReader`]. The
/// stream's error type only needs to be convertible into a boxed error.
pub fn stream_to_reader<S, E>(stream: S) -> impl AsyncRead + Unpin + Send
where
    S: Stream<Item = Result<Bytes, E>> + Unpin + Send,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    StreamReader::new(stream.map_err(io::Error::other))
}

/// Adapts an [`AsyncRead`] into a byte stream of `Bytes`, surfacing read errors
/// as [`io::Error`]. Callers map that into their own error type as needed.
pub fn reader_to_stream<R>(reader: R) -> impl Stream<Item = Result<Bytes, io::Error>> + Unpin + Send
where
    R: AsyncRead + Unpin + Send,
{
    ReaderStream::new(reader)
}

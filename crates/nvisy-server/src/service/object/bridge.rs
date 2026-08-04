//! Adapters between object-store byte streams and `AsyncRead`.
//!
//! The object store yields `Stream<Item = Result<Bytes, _>>` while the NATS
//! object store consumes and produces `AsyncRead`. These helpers convert
//! between the two so an object can be piped end to end without buffering the
//! whole body in memory.

use std::io;

use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use nvisy_object::error::Error as ObjectError;
use tokio::io::AsyncRead;
use tokio_util::io::{ReaderStream, StreamReader};

/// Adapts a byte stream from the object store into an [`AsyncRead`].
///
/// Stream errors surface as [`io::Error`], as required by [`StreamReader`].
pub fn stream_to_reader<S>(stream: S) -> impl AsyncRead + Unpin + Send
where
    S: Stream<Item = Result<Bytes, ObjectError>> + Unpin + Send,
{
    StreamReader::new(stream.map_err(io::Error::other))
}

/// Adapts an [`AsyncRead`] into a byte stream the object store can upload.
///
/// I/O errors surface as an object-store [`ObjectError`], as required by the
/// multipart upload API.
pub fn reader_to_stream<R>(
    reader: R,
) -> impl Stream<Item = Result<Bytes, ObjectError>> + Unpin + Send
where
    R: AsyncRead + Unpin + Send,
{
    // The reader is the decrypt pipeline over stored bytes, so an error here is
    // a read/decrypt failure of the source object rather than a store fault.
    ReaderStream::new(reader).map_err(|e| ObjectError::runtime(e, "source-read"))
}

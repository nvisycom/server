//! Streamed file transfer types shared by the object-store and cloud
//! file-service clients.

use bytes::Bytes;
use futures::stream::BoxStream;

use crate::Result;

/// A byte stream, the shape both directions of a transfer move data in.
pub type ByteStream = BoxStream<'static, Result<Bytes>>;

/// A streamed upload: the destination key, its content type, the exact byte
/// length, and the body.
///
/// `content_length` must equal the bytes `body` yields. A file service that
/// needs the length up front (Dropbox rejects a chunked body) sends it as the
/// request `Content-Length`, and a mismatch fails the upload; an object store
/// streams via multipart and does not need it.
#[must_use]
pub struct FileUpload<'a> {
    /// The destination: an object path, or the name of the new file a file
    /// service creates.
    pub key: &'a str,
    /// The MIME type of the content.
    pub content_type: &'a str,
    /// The exact number of bytes `body` yields.
    pub content_length: u64,
    /// The file's bytes.
    pub body: ByteStream,
}

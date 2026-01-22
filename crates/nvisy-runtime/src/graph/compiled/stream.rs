//! Stream types for compiled workflow data flow.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::BoxStream;
use futures::{Sink, SinkExt, Stream, StreamExt};
use nvisy_dal::AnyDataValue;

use crate::error::{Error, Result};

/// A boxed stream of workflow data values.
pub type DataStream = BoxStream<'static, Result<AnyDataValue>>;

/// A boxed sink for workflow data values.
pub type DataSink = Pin<Box<dyn Sink<AnyDataValue, Error = Error> + Send + 'static>>;

/// Input stream for reading data in a workflow.
///
/// Wraps a boxed stream and provides metadata about the source.
pub struct InputStream {
    /// The underlying data stream.
    stream: DataStream,
    /// Optional cursor for pagination.
    cursor: Option<String>,
    /// Optional limit on items to read.
    limit: Option<usize>,
    /// Number of items read so far.
    items_read: usize,
}

impl InputStream {
    /// Creates a new input stream.
    pub fn new(stream: DataStream) -> Self {
        Self {
            stream,
            cursor: None,
            limit: None,
            items_read: 0,
        }
    }

    /// Creates an input stream with a cursor for pagination.
    pub fn with_cursor(stream: DataStream, cursor: impl Into<String>) -> Self {
        Self {
            stream,
            cursor: Some(cursor.into()),
            limit: None,
            items_read: 0,
        }
    }

    /// Creates an input stream with a limit on items to read.
    pub fn with_limit(stream: DataStream, limit: usize) -> Self {
        Self {
            stream: Box::pin(stream.take(limit)),
            cursor: None,
            limit: Some(limit),
            items_read: 0,
        }
    }

    /// Creates an input stream with both cursor and limit.
    pub fn with_cursor_and_limit(
        stream: DataStream,
        cursor: impl Into<String>,
        limit: usize,
    ) -> Self {
        Self {
            stream: Box::pin(stream.take(limit)),
            cursor: Some(cursor.into()),
            limit: Some(limit),
            items_read: 0,
        }
    }

    /// Returns the cursor for the next page, if any.
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Returns the limit on items to read, if set.
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the number of items read so far.
    pub fn items_read(&self) -> usize {
        self.items_read
    }

    /// Consumes the stream and returns the inner boxed stream.
    pub fn into_inner(self) -> DataStream {
        self.stream
    }

    /// Consumes the stream and returns all parts.
    pub fn into_parts(self) -> (DataStream, Option<String>, Option<usize>) {
        (self.stream, self.cursor, self.limit)
    }
}

impl Stream for InputStream {
    type Item = Result<AnyDataValue>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.stream).poll_next(cx);
        if let Poll::Ready(Some(Ok(_))) = &result {
            self.items_read += 1;
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl std::fmt::Debug for InputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputStream")
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .field("items_read", &self.items_read)
            .finish_non_exhaustive()
    }
}

/// Output stream for writing data in a workflow.
///
/// Wraps a boxed sink and tracks write statistics.
pub struct OutputStream {
    /// The underlying data sink.
    sink: DataSink,
    /// Optional buffer size for batching.
    buffer_size: Option<usize>,
    /// Number of items written so far.
    items_written: usize,
}

impl OutputStream {
    /// Creates a new output stream.
    pub fn new(sink: DataSink) -> Self {
        Self {
            sink,
            buffer_size: None,
            items_written: 0,
        }
    }

    /// Creates an output stream with buffering for batched writes.
    pub fn with_buffer(sink: DataSink, buffer_size: usize) -> Self {
        Self {
            sink: Box::pin(sink.buffer(buffer_size)),
            buffer_size: Some(buffer_size),
            items_written: 0,
        }
    }

    /// Returns the buffer size, if set.
    pub fn buffer_size(&self) -> Option<usize> {
        self.buffer_size
    }

    /// Returns the number of items written so far.
    pub fn items_written(&self) -> usize {
        self.items_written
    }

    /// Consumes the stream and returns the inner boxed sink.
    pub fn into_inner(self) -> DataSink {
        self.sink
    }
}

impl Sink<AnyDataValue> for OutputStream {
    type Error = Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        self.sink.as_mut().poll_ready(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: AnyDataValue,
    ) -> std::result::Result<(), Self::Error> {
        self.items_written += 1;
        self.sink.as_mut().start_send(item)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        self.sink.as_mut().poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        self.sink.as_mut().poll_close(cx)
    }
}

impl std::fmt::Debug for OutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputStream")
            .field("buffer_size", &self.buffer_size)
            .field("items_written", &self.items_written)
            .finish_non_exhaustive()
    }
}

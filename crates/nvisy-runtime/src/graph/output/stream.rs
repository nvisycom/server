//! Output stream types for compiled workflow data flow.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Sink, SinkExt};
use nvisy_dal::datatype::AnyDataValue;

use crate::error::Error;

/// A boxed sink for workflow data values.
pub type DataSink = Pin<Box<dyn Sink<AnyDataValue, Error = Error> + Send + 'static>>;

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

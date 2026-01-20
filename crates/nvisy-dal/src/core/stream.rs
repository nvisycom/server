//! Stream types for data input and output operations.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::BoxStream;
use futures::{Sink, Stream};

use crate::Result;

/// A boxed stream of items.
pub type ItemStream<'a, T> = BoxStream<'a, Result<T>>;

/// Input stream wrapper for reading data.
///
/// Wraps a boxed stream and provides a cursor for pagination.
pub struct InputStream<'a, T> {
    stream: ItemStream<'a, T>,
    cursor: Option<String>,
}

impl<'a, T> InputStream<'a, T> {
    /// Creates a new input stream.
    pub fn new(stream: ItemStream<'a, T>) -> Self {
        Self {
            stream,
            cursor: None,
        }
    }

    /// Creates a new input stream with a cursor.
    pub fn with_cursor(stream: ItemStream<'a, T>, cursor: Option<String>) -> Self {
        Self { stream, cursor }
    }

    /// Returns the cursor for the next read, if any.
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Consumes the stream and returns the inner boxed stream.
    pub fn into_inner(self) -> ItemStream<'a, T> {
        self.stream
    }

    /// Consumes the stream and returns both the inner stream and cursor.
    pub fn into_parts(self) -> (ItemStream<'a, T>, Option<String>) {
        (self.stream, self.cursor)
    }
}

impl<T> Stream for InputStream<'_, T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

/// A boxed sink for items.
pub type ItemSink<'a, T> = Pin<Box<dyn Sink<T, Error = crate::Error> + Send + 'a>>;

/// Output stream wrapper for writing data.
///
/// Wraps a boxed sink for streaming writes.
pub struct OutputStream<'a, T> {
    sink: ItemSink<'a, T>,
}

impl<'a, T> OutputStream<'a, T> {
    /// Creates a new output stream.
    pub fn new(sink: ItemSink<'a, T>) -> Self {
        Self { sink }
    }

    /// Consumes the stream and returns the inner boxed sink.
    pub fn into_inner(self) -> ItemSink<'a, T> {
        self.sink
    }
}

impl<T> Sink<T> for OutputStream<'_, T> {
    type Error = crate::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.sink.as_mut().poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<()> {
        self.sink.as_mut().start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.sink.as_mut().poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.sink.as_mut().poll_close(cx)
    }
}

impl<T> std::fmt::Debug for OutputStream<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputStream").finish_non_exhaustive()
    }
}

impl<T> std::fmt::Debug for InputStream<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputStream")
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

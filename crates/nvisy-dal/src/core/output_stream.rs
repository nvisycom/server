//! Output stream types for writing data.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Sink;

use crate::Error;

/// A boxed sink for items with a lifetime.
pub type ItemSink<'a, T> = Pin<Box<dyn Sink<T, Error = Error> + Send + 'a>>;

/// Output stream wrapper for writing data.
///
/// Wraps a boxed sink for streaming writes.
pub struct OutputStream<T> {
    sink: ItemSink<'static, T>,
}

impl<T> OutputStream<T> {
    /// Creates a new output stream.
    pub fn new(sink: ItemSink<'static, T>) -> Self {
        Self { sink }
    }

    /// Consumes the stream and returns the inner boxed sink.
    pub fn into_inner(self) -> ItemSink<'static, T> {
        self.sink
    }
}

impl<T> Sink<T> for OutputStream<T> {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink.as_mut().poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.sink.as_mut().start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink.as_mut().poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink.as_mut().poll_close(cx)
    }
}

impl<T> std::fmt::Debug for OutputStream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputStream").finish_non_exhaustive()
    }
}

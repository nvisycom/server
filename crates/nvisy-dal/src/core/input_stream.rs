//! Input stream types for reading data.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use futures::stream::BoxStream;

use crate::Result;

/// A boxed stream of items with a lifetime.
pub type ItemStream<'a, T> = BoxStream<'a, Result<T>>;

/// Input stream wrapper for reading data.
pub struct InputStream<T> {
    stream: ItemStream<'static, T>,
}

impl<T> InputStream<T> {
    /// Creates a new input stream.
    pub fn new(stream: ItemStream<'static, T>) -> Self {
        Self { stream }
    }

    /// Consumes the stream and returns the inner boxed stream.
    pub fn into_inner(self) -> ItemStream<'static, T> {
        self.stream
    }
}

impl<T> Stream for InputStream<T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<T> std::fmt::Debug for InputStream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputStream").finish_non_exhaustive()
    }
}

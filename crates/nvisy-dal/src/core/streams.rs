//! Stream types for reading and writing data.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::BoxStream;
use futures::{Sink, Stream, StreamExt};

use crate::contexts::AnyContext;
use crate::datatypes::AnyDataValue;
use crate::{Error, Result, Resumable};

/// A boxed stream of items with a lifetime.
pub type ItemStream<'a, T> = BoxStream<'a, Result<T>>;

/// A boxed sink for items with a lifetime.
pub type ItemSink<'a, T> = Pin<Box<dyn Sink<T, Error = Error> + Send + 'a>>;

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

/// Extension trait for converting typed input streams to type-erased streams.
pub trait InputStreamExt<T, C> {
    /// Converts this stream to a type-erased stream with `AnyDataValue` and `AnyContext`.
    fn into_any(self) -> InputStream<Resumable<AnyDataValue, AnyContext>>
    where
        T: Into<AnyDataValue> + 'static,
        C: Into<AnyContext> + 'static;
}

impl<T, C> InputStreamExt<T, C> for InputStream<Resumable<T, C>>
where
    T: Send,
    C: Send,
{
    fn into_any(self) -> InputStream<Resumable<AnyDataValue, AnyContext>>
    where
        T: Into<AnyDataValue> + 'static,
        C: Into<AnyContext> + 'static,
    {
        let mapped =
            self.map(move |r| r.map(|item| Resumable::new(item.data.into(), item.context.into())));
        InputStream::new(Box::pin(mapped))
    }
}

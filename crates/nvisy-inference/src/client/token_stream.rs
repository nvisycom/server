//! The [`TokenStream`] type: an assistant response streamed as text deltas.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use futures::stream::{BoxStream, StreamExt};

use crate::error::Error;

/// A stream of the assistant's response as text deltas.
///
/// Each item is a token chunk (`Ok`); a failure mid-generation is the final item
/// (`Err`) and ends the stream. Owned and `'static`, so it can be moved into a
/// response body outliving the client that produced it.
///
/// Yielded by [`InferenceClient::stream_chat`](crate::InferenceClient::stream_chat).
/// Poll it with the [`Stream`] API ([`futures::StreamExt`]).
#[must_use = "a token stream does nothing unless polled"]
pub struct TokenStream {
    inner: BoxStream<'static, Result<String, Error>>,
}

impl TokenStream {
    /// Wraps an owned delta stream.
    pub(crate) fn new(inner: BoxStream<'static, Result<String, Error>>) -> Self {
        Self { inner }
    }
}

impl Stream for TokenStream {
    type Item = Result<String, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

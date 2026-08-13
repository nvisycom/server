//! A response body that counts the bytes streamed through it.
//!
//! Axum responses are streaming bodies without a `Content-Length` header at the
//! point request middleware inspects them, so reading the header yields no size.
//! [`CountingBody`] wraps a body and tallies each data frame's length as it is
//! polled, invoking a callback with the total once the stream ends — by reaching
//! its last frame or by being dropped early (a client disconnect mid-stream).
//!
//! It never buffers: frames pass through untouched, so it is safe for large
//! downloads and long-lived event streams alike. The trade-off is timing — the
//! total is known only when the body finishes, which for a streamed response is
//! after the handler has returned.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;

pin_project! {
    /// Wraps a body, counting streamed bytes and reporting the total on end.
    ///
    /// The callback fires exactly once: on the last frame, or on drop if the
    /// body is dropped before completing (so an interrupted response is still
    /// reported, with the bytes sent before the interruption).
    #[project = CountingBodyProj]
    pub struct CountingBody<B, F>
    where
        F: FnOnce(u64),
    {
        #[pin]
        inner: B,
        bytes: u64,
        // `Some` until the callback fires; taken so it runs at most once, whether
        // that is at end-of-stream or on drop.
        on_end: Option<F>,
    }

    impl<B, F> PinnedDrop for CountingBody<B, F>
    where
        F: FnOnce(u64),
    {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(on_end) = this.on_end.take() {
                on_end(*this.bytes);
            }
        }
    }
}

impl<B, F> CountingBody<B, F>
where
    F: FnOnce(u64),
{
    /// Wraps `inner`, calling `on_end` with the total byte count when the body
    /// finishes streaming (or is dropped).
    pub fn new(inner: B, on_end: F) -> Self {
        Self {
            inner,
            bytes: 0,
            on_end: Some(on_end),
        }
    }
}

impl<B, F> Body for CountingBody<B, F>
where
    B: Body,
    F: FnOnce(u64),
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let polled = this.inner.poll_frame(cx);

        match &polled {
            // A data frame: add its length to the running total.
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    *this.bytes += data.remaining() as u64;
                }
            }
            // End of stream: report the total once (drop reports it otherwise).
            Poll::Ready(None) => {
                if let Some(on_end) = this.on_end.take() {
                    on_end(*this.bytes);
                }
            }
            _ => {}
        }

        polled
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::future::poll_fn;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use axum::body::Body as AxumBody;
    use http_body::Body as HttpBody;

    use super::*;

    /// Drives a body to completion, returning the total bytes across its data
    /// frames.
    async fn drain<B: HttpBody + Unpin>(mut body: B) -> u64
    where
        B::Error: std::fmt::Debug,
    {
        let mut total = 0;
        while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            if let Some(data) = frame.unwrap().data_ref() {
                total += data.remaining() as u64;
            }
        }
        total
    }

    /// Draining the body to completion reports the exact byte total once.
    #[tokio::test]
    async fn counts_streamed_bytes_on_completion() {
        let reported = Arc::new(AtomicU64::new(u64::MAX));
        let sink = Arc::clone(&reported);

        let body = CountingBody::new(AxumBody::from("hello world"), move |n| {
            sink.store(n, Ordering::SeqCst)
        });

        assert_eq!(drain(body).await, 11);
        assert_eq!(reported.load(Ordering::SeqCst), 11);
    }

    /// An empty body reports zero — distinct from the previous "header absent"
    /// fallback that also produced zero for non-empty responses.
    #[tokio::test]
    async fn counts_zero_for_empty_body() {
        let reported = Arc::new(AtomicU64::new(u64::MAX));
        let sink = Arc::clone(&reported);

        let body = CountingBody::new(AxumBody::empty(), move |n| sink.store(n, Ordering::SeqCst));
        assert_eq!(drain(body).await, 0);
        assert_eq!(reported.load(Ordering::SeqCst), 0);
    }

    /// Dropping the body before it is polled still reports (zero bytes seen)
    /// exactly once, via the drop path rather than the end-of-stream path.
    #[tokio::test]
    async fn reports_on_early_drop() {
        let reported = Arc::new(AtomicU64::new(u64::MAX));
        let sink = Arc::clone(&reported);

        // `CountingBody` is `Unpin` here, so dropping the owned value runs its
        // `PinnedDrop` and fires the callback with the bytes seen so far (none).
        let body = CountingBody::new(AxumBody::from("partial"), move |n| {
            sink.store(n, Ordering::SeqCst)
        });
        drop(body);

        assert_eq!(reported.load(Ordering::SeqCst), 0);
    }
}

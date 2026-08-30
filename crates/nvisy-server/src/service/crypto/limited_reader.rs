//! An [`AsyncRead`] wrapper that fails once more than a byte budget is read.
//!
//! Placed ahead of the upload pipe's hashing and encryption stages, it aborts an
//! oversized stream as soon as the limit is crossed — before the excess is
//! encrypted and written to storage — rather than measuring the size only after
//! the whole body has streamed through.
//!
//! The failing read raises an [`io::Error`], but intervening stages (the
//! encryptor, the object store) may stringify that error and lose its cause. So
//! the reader also records the trip in a shared [`LimitState`] handle the caller
//! can consult afterwards to tell an over-limit abort apart from a genuine I/O or
//! storage failure.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, ReadBuf};

/// A shared handle to whether a [`LimitedReader`] exceeded its budget.
///
/// Cloneable and readable after the reader has been consumed, which is when the
/// caller — whose downstream `put`/encrypt may have swallowed the reader's error
/// into a generic failure — needs to know the cause was the size limit.
#[derive(Clone, Default)]
pub struct LimitState {
    exceeded: Arc<AtomicBool>,
}

impl LimitState {
    /// Whether the reader read past its budget.
    pub fn is_exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Relaxed)
    }
}

pin_project! {
    /// An [`AsyncRead`] that yields an error once its byte budget is exceeded.
    ///
    /// Reads pass through untouched until the cumulative total would exceed
    /// `limit`; the read that crosses the budget marks the shared [`LimitState`]
    /// and fails with an [`io::Error`]. A stream that stays within the budget is
    /// unaffected.
    pub struct LimitedReader<R> {
        #[pin]
        inner: R,
        limit: u64,
        read: u64,
        state: LimitState,
    }
}

impl<R> LimitedReader<R> {
    /// Wraps `inner`, allowing at most `limit` bytes, and returns it alongside a
    /// handle that reports whether the budget was exceeded.
    pub fn new(inner: R, limit: u64) -> (Self, LimitState) {
        let state = LimitState::default();
        let reader = Self {
            inner,
            limit,
            read: 0,
            state: state.clone(),
        };
        (reader, state)
    }
}

impl<R: AsyncRead> AsyncRead for LimitedReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let before = buf.filled().len();
        let poll = this.inner.poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            *this.read += (buf.filled().len() - before) as u64;
            if *this.read > *this.limit {
                this.state.exceeded.store(true, Ordering::Relaxed);
                // Roll the just-read bytes back out of the buffer: an `AsyncRead`
                // that returns an error must not also report bytes read on the
                // same poll (tokio's read helpers assert this).
                buf.set_filled(before);
                return Poll::Ready(Err(io::Error::other(format!(
                    "upload exceeds the {}-byte limit",
                    *this.limit
                ))));
            }
        }
        poll
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::LimitedReader;

    #[tokio::test]
    async fn reads_within_the_limit_pass_through() {
        let data = [7u8; 64];
        let (mut reader, state) = LimitedReader::new(&data[..], 64);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await.expect("within limit");
        assert_eq!(out, data);
        assert!(!state.is_exceeded());
    }

    #[tokio::test]
    async fn reading_past_the_limit_fails_and_marks_the_state() {
        let data = [7u8; 65];
        let (mut reader, state) = LimitedReader::new(&data[..], 64);
        let mut out = Vec::new();
        reader
            .read_to_end(&mut out)
            .await
            .expect_err("over limit must fail");
        assert!(state.is_exceeded());
    }
}
